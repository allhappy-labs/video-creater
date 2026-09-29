import { inProcessExportLabel } from "@/lib/export/profiles";
import { generatedAssetTitleWithPrompt } from "@/lib/generation/assets";
import {
  buildActivityJobRecords,
  formatJobKindSentenceCase,
  generationJobAssetId,
  startRequestStringInput,
  type ActivityJobRecord,
} from "@/lib/jobs/activity-records";
import { isSaveRangeJobId, videoExportJobIdProfile } from "@/lib/jobs/ids";
import { mediaDisplayName } from "@/lib/media/names";
import type { GeneratedAsset, ProjectJobSummary, VideoProject } from "@/lib/project";

export type TaskKind = "transcription" | "analysis" | "generation" | "render" | "export" | "agent";
type TaskStatus = "queued" | "running" | "completed" | "failed" | "cancelled" | "blocked";
type TaskBackend = "temporal" | "inProcess";

export interface TaskRecord {
  id: string;
  kind: TaskKind;
  label: string;
  status: TaskStatus;
  /**
   * 0–1 for a running task whose job reports progress (renders, exports and xAI video generations write
   * lease-free snapshots the editor polls); null when the task isn't running or reports none.
   */
  progress: number | null;
  detail: string | null;
  failureReason: string | null;
  artifactPath: string | null;
  logPath: string | null;
  cancel: { available: true } | { available: false; reason: string };
  retry: boolean;
  updatedAt: string;
  workflow: { runId?: string; workflowId?: string; backend: TaskBackend } | null;
}

/** The in-flight agent turn, owned by the agent slice. */
interface AgentTurnState {
  readonly id: string;
  readonly label: string;
  readonly status: "running" | "completed" | "failed" | "cancelled";
  /** The current progress phase, e.g. "Reviewing the timeline". */
  readonly phase: string | null;
  readonly failureReason: string | null;
  readonly updatedAt: string;
}

/** In-memory editor state that the project file does not record. */
export interface TaskRuntime {
  readonly projectDir: string;
  /** The `generationExecutionBackend` preference, deciding how a queued generation will run. */
  readonly executionBackend: TaskBackend;
  /** The in-process render started in this session (its cancel needs the attempt id). */
  readonly activeRender: { readonly jobId: string; readonly attemptId: string; readonly startedAt: string } | null;
  readonly agentTurn: AgentTurnState | null;
  /** Jobs with a stored export plan; a `render_draft` job among them is a video export. */
  readonly exportPlanJobIds: ReadonlySet<string>;
  /** The latest progress snapshot (0–1) per job id; never stored in the project or undo history. */
  readonly progressByJobId: ReadonlyMap<string, number>;
  /** Why the last Temporal reconciliation couldn't use the workflow service; null when it could. */
  readonly workflowServiceIssue: string | null;
}

export const temporalCancelReason = "This task runs in the workflow worker and can't be cancelled from the editor.";
const finishedCancelReason = "This task is no longer running.";
const unsupportedCancelReason = "This task can't be cancelled from the editor.";

const activeStatuses: ReadonlySet<TaskStatus> = new Set(["queued", "running", "blocked"]);
const terminalStatuses: ReadonlySet<string> = new Set(["completed", "failed", "cancelled"]);

const runningDetail: Record<TaskKind, string> = {
  transcription: "Transcribing…",
  analysis: "Analyzing…",
  generation: "Generating…",
  render: "Rendering…",
  export: "Exporting…",
  agent: "Working…",
};

const failureReasons: Record<TaskKind, string> = {
  transcription: "The transcription stopped before it finished.",
  analysis: "The analysis stopped before it finished.",
  generation: "The generation stopped before it finished.",
  render: "The render stopped before it finished.",
  export: "The export stopped before it finished.",
  agent: "The agent couldn't finish this edit.",
};

/** The workflow service problem shown when reconciliation reports no detail of its own. */
export const workflowServiceUnreachableDetail = "Workflow service unreachable";

const downloadFailureReason = "The generated file couldn't be downloaded.";
const saveRangeFailureReason = "The range couldn't be saved to Media.";

/**
 * A stored plan or an export job id makes a render an export (the plan is gone after a restart),
 * except a saved range, whose plan only serves Retry.
 */
function renderKind(jobId: string, runtime: TaskRuntime): TaskKind {
  const exported = runtime.exportPlanJobIds.has(jobId) || videoExportJobIdProfile(jobId) !== null;
  return exported && !isSaveRangeJobId(jobId) ? "export" : "render";
}

function renderLabel(jobId: string): string {
  return isSaveRangeJobId(jobId) ? "Timeline range render" : "Timeline render";
}

function taskKind(job: ProjectJobSummary, runtime: TaskRuntime): TaskKind {
  const kind = job.kind.toLowerCase();
  if (kind.includes("transcri")) return "transcription";
  if (kind.includes("generat") || kind.includes("upscale")) return "generation";
  if (kind.includes("codex") || kind.includes("agent")) return "agent";
  if (kind.includes("export")) return "export";
  if (kind.includes("render")) return renderKind(job.id, runtime);
  return "analysis";
}

function taskStatus(status: ProjectJobSummary["status"]): TaskStatus {
  return status === "progress" ? "running" : status;
}

/** Run ids of local work; Rust reconciliation (`LOCAL_RUN_ID_PREFIXES`) skips the same ones. */
const localRunIdPrefixes = ["in-process/", "render-attempt/", "mock-run-"] as const;

function taskBackend(job: ProjectJobSummary, runtime: TaskRuntime): TaskBackend {
  const runId = job.workflow?.runId ?? null;
  if (runId && localRunIdPrefixes.some((prefix) => runId.startsWith(prefix))) return "inProcess";
  if (runtime.activeRender?.jobId === job.id) return "inProcess";
  if (runId) return "temporal";
  // Nothing has started yet: in-process renders and exports record no start request.
  if (!job.startRequest) return "inProcess";
  return job.kind === "generate_media" ? runtime.executionBackend : "temporal";
}

function workflowOf(job: ProjectJobSummary, backend: TaskBackend): NonNullable<TaskRecord["workflow"]> {
  const runId = job.workflow?.runId ?? null;
  const workflowId = job.workflow?.workflowId ?? job.startRequest?.workflowId ?? null;
  return { ...(runId ? { runId } : {}), ...(workflowId ? { workflowId } : {}), backend };
}

/** Legacy rule: a real (non-mock) in-process generation that is running and has its asset. */
function cancellableGeneration(job: ProjectJobSummary, project: VideoProject, runtime: TaskRuntime): boolean {
  return (
    runtime.projectDir.trim().length > 0 &&
    (job.status === "running" || job.status === "progress") &&
    job.kind === "generate_media" &&
    Boolean(job.workflow?.runId?.startsWith("in-process/")) &&
    job.startRequest?.input.mockMode === false &&
    project.generatedAssets.some((asset) => asset.id === generationJobAssetId(job))
  );
}

function cancelOf(job: ProjectJobSummary, status: TaskStatus, backend: TaskBackend, project: VideoProject, runtime: TaskRuntime): TaskRecord["cancel"] {
  if (!activeStatuses.has(status)) return { available: false, reason: finishedCancelReason };
  if (runtime.activeRender?.jobId === job.id || cancellableGeneration(job, project, runtime)) return { available: true };
  return { available: false, reason: backend === "temporal" ? temporalCancelReason : unsupportedCancelReason };
}

/** A completed generation whose output media is missing (generated output media cannot be deleted). */
function downloadFailed(asset: GeneratedAsset, project: VideoProject): boolean {
  return (
    asset.status === "completed" &&
    asset.outputs.some((output) => !project.media.some((media) => media.id === output.mediaId))
  );
}

function canRetryDownload(asset: GeneratedAsset, project: VideoProject): boolean {
  return asset.outputs.some((output) => !project.media.some((media) => media.id === output.mediaId) && Boolean(output.sourceUrl?.trim()));
}

function exportLabel(job: ProjectJobSummary): string {
  const format = startRequestStringInput(job, "format");
  if (job.kind === "export_nle_xml" || format) {
    if (format === "premiereXmeml") return "Premiere XML export";
    if (format === "davinciFcpxml") return "DaVinci XML export";
    return "Timeline XML export";
  }
  const settings = job.exportSettings;
  if (settings) return `${inProcessExportLabel(settings.profile, settings.quality, settings.encodeTier)} export`;
  const profile = startRequestStringInput(job, "profile");
  if (profile === "palmierProject") return "Palmier Project package";
  const quality = startRequestStringInput(job, "quality");
  if ((profile === "webm" || profile === "mp4H264" || profile === "mp4H265" || profile === "proResMov") && (quality === "draft" || quality === "final")) {
    return `${inProcessExportLabel(profile, quality, startRequestStringInput(job, "encodeTier") === "master" ? "master" : undefined)} export`;
  }
  return "Video export";
}

function jobLabel(job: ProjectJobSummary, kind: TaskKind, project: VideoProject, asset: GeneratedAsset | null): string {
  switch (kind) {
    case "transcription": {
      const mediaId = startRequestStringInput(job, "mediaId");
      const media = project.media.find((candidate) => candidate.id === mediaId);
      return media ? `Transcribe ${mediaDisplayName(media)}` : "Transcribe media";
    }
    case "generation":
      return asset ? generatedAssetTitleWithPrompt(asset) : "Generate media";
    case "render":
      return renderLabel(job.id);
    case "export":
      return exportLabel(job);
    case "agent":
      return "Agent edit";
    case "analysis":
      return formatJobKindSentenceCase(job.kind);
  }
}

function detailOf(kind: TaskKind, status: TaskStatus): string | null {
  switch (status) {
    case "queued":
      return "Queued";
    case "blocked":
      return "Waiting to start";
    case "running":
      return runningDetail[kind];
    case "completed":
      return "Completed";
    case "cancelled":
      return "Cancelled";
    case "failed":
      return null;
  }
}

function jobFailureReason(job: ProjectJobSummary, kind: TaskKind, downloadFailure: boolean): string {
  if (downloadFailure) return downloadFailureReason;
  const recorded = job.failureReason?.trim();
  if (recorded) return recorded;
  return kind === "render" && isSaveRangeJobId(job.id) ? saveRangeFailureReason : failureReasons[kind];
}

function runningProgress(jobId: string, status: TaskStatus, runtime: TaskRuntime): number | null {
  return status === "running" ? (runtime.progressByJobId.get(jobId) ?? null) : null;
}

function jobDetail(kind: TaskKind, status: TaskStatus, backend: TaskBackend, runtime: TaskRuntime): string | null {
  if (runtime.workflowServiceIssue && backend === "temporal" && (status === "queued" || status === "running")) {
    return runtime.workflowServiceIssue;
  }
  return detailOf(kind, status);
}

function jobTaskRecord(record: ActivityJobRecord, project: VideoProject, runtime: TaskRuntime): TaskRecord {
  const { job } = record;
  const kind = taskKind(job, runtime);
  const asset = kind === "generation" ? (project.generatedAssets.find((candidate) => candidate.id === generationJobAssetId(job)) ?? null) : null;
  const downloadFailure = asset !== null && downloadFailed(asset, project);
  const jobStatus = taskStatus(job.status);
  const assetTerminal = asset !== null && terminalStatuses.has(asset.status) && !terminalStatuses.has(jobStatus);
  const status: TaskStatus = downloadFailure ? "failed" : assetTerminal && asset ? asset.status : jobStatus;
  const backend = taskBackend(job, runtime);
  const failed = status === "failed";
  return {
    id: job.id,
    kind,
    label: jobLabel(job, kind, project, asset),
    status,
    progress: runningProgress(job.id, status, runtime),
    detail: jobDetail(kind, status, backend, runtime),
    failureReason: failed ? jobFailureReason(job, kind, downloadFailure) : null,
    artifactPath: status === "completed" ? record.outputPath : null,
    logPath: record.logPath,
    cancel: cancelOf(job, status, backend, project, runtime),
    retry: failed && retryable(job, kind, project, asset, downloadFailure),
    updatedAt: job.updatedAt,
    workflow: workflowOf(job, backend),
  };
}

function retryable(job: ProjectJobSummary, kind: TaskKind, project: VideoProject, asset: GeneratedAsset | null, downloadFailure: boolean): boolean {
  switch (kind) {
    case "generation":
      return Boolean(asset && (asset.prompt.trim() || (downloadFailure && canRetryDownload(asset, project))));
    case "render":
      return !job.startRequest;
    case "export":
      // Re-runs the stored export plan, or opens the Export popover preset from the job.
      return true;
    case "transcription": {
      const mediaId = startRequestStringInput(job, "mediaId");
      return project.media.some((media) => media.id === mediaId);
    }
    case "analysis":
    case "agent":
      return false;
  }
}

function assetTaskRecord(asset: GeneratedAsset, project: VideoProject): TaskRecord {
  const downloadFailure = downloadFailed(asset, project);
  const status: TaskStatus = downloadFailure ? "failed" : asset.status;
  const failed = status === "failed";
  return {
    id: asset.id,
    kind: "generation",
    label: generatedAssetTitleWithPrompt(asset),
    status,
    progress: null,
    detail: detailOf("generation", status),
    failureReason: failed ? (downloadFailure ? downloadFailureReason : failureReasons.generation) : null,
    artifactPath: status === "completed" ? (asset.outputs[0]?.relativePath.trim() || null) : null,
    logPath: null,
    cancel: { available: false, reason: activeStatuses.has(status) ? unsupportedCancelReason : finishedCancelReason },
    retry: failed && Boolean(asset.prompt.trim() || (downloadFailure && canRetryDownload(asset, project))),
    updatedAt: asset.createdAt,
    workflow: null,
  };
}

function activeRenderRecord(render: NonNullable<TaskRuntime["activeRender"]>, runtime: TaskRuntime): TaskRecord {
  const kind = renderKind(render.jobId, runtime);
  return {
    id: render.jobId,
    kind,
    label: kind === "export" ? "Video export" : renderLabel(render.jobId),
    status: "running",
    progress: runtime.progressByJobId.get(render.jobId) ?? null,
    detail: runningDetail[kind],
    failureReason: null,
    artifactPath: null,
    logPath: null,
    cancel: { available: true },
    retry: false,
    updatedAt: render.startedAt,
    workflow: { runId: render.attemptId, backend: "inProcess" },
  };
}

function agentTurnRecord(turn: AgentTurnState): TaskRecord {
  const running = turn.status === "running";
  return {
    id: turn.id,
    kind: "agent",
    label: turn.label,
    status: turn.status,
    progress: null,
    detail: running ? (turn.phase ?? runningDetail.agent) : detailOf("agent", turn.status),
    failureReason: turn.status === "failed" ? (turn.failureReason ?? failureReasons.agent) : null,
    artifactPath: null,
    logPath: null,
    cancel: running ? { available: true } : { available: false, reason: finishedCancelReason },
    retry: false,
    updatedAt: turn.updatedAt,
    workflow: null,
  };
}

const statusGroup: Record<TaskStatus, number> = { running: 0, queued: 1, blocked: 1, failed: 2, completed: 3, cancelled: 3 };

function updatedAtMs(record: TaskRecord): number {
  const timestamp = Date.parse(record.updatedAt);
  return Number.isFinite(timestamp) ? timestamp : Number.NEGATIVE_INFINITY;
}

/**
 * Canonical preview frame captures (AI result frames, effect previews) record a job, but they are
 * preview plumbing the requesting surface reports itself, not background work the user started.
 */
const previewCaptureJobKind = "captureCanonicalPreviewFrame";

/**
 * Background task records for every job except preview captures, generated asset without a job,
 * in-process render not yet recorded, and agent turn. Ordered running, queued (and blocked), failed,
 * then completed (and cancelled); newest `updatedAt` first within each group.
 */
export function taskRecords(project: VideoProject, runtime: TaskRuntime): TaskRecord[] {
  const jobIds = new Set(project.jobs.map((job) => job.id));
  const assetIdsWithJobs = new Set(project.jobs.map(generationJobAssetId));
  const records = [
    ...buildActivityJobRecords(project)
      .filter((record) => record.job.kind !== previewCaptureJobKind)
      .map((record) => jobTaskRecord(record, project, runtime)),
    ...project.generatedAssets.filter((asset) => !assetIdsWithJobs.has(asset.id)).map((asset) => assetTaskRecord(asset, project)),
  ];
  if (runtime.activeRender && !jobIds.has(runtime.activeRender.jobId)) records.push(activeRenderRecord(runtime.activeRender, runtime));
  if (runtime.agentTurn && !jobIds.has(runtime.agentTurn.id)) records.push(agentTurnRecord(runtime.agentTurn));
  return records
    .map((record, index) => ({ record, index }))
    .sort((left, right) => statusGroup[left.record.status] - statusGroup[right.record.status] || updatedAtMs(right.record) - updatedAtMs(left.record) || left.index - right.index)
    .map(({ record }) => record);
}
