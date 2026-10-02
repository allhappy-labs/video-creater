import { exportFileLabel } from "../../export/export-naming";
import { mediaExportOutputPath } from "../../export/profiles";
import type {
  ExportEncodeTier,
  ExportOutput,
  ExportProfile,
  ExportProfileAvailability,
  ExportQuality,
  JobExportSettings,
  JobProgressSnapshot,
  NleXmlExportCommandResult,
  NleXmlExportFormat,
  ProjectActionWriteResult,
  ProjectExportArtifact,
  ProjectJobSummary,
  ProjectMediaRenderResult,
  MediaRenderAttempt,
  ProjectRenderReport,
  VideoProject,
} from "../../project";
import { buildSampleRenderReport, type RenderReport } from "../../render";
import type { FixtureOperationHandler } from "../adapters/fixture-transport";
import { fixtureWriteReport, type FixtureProjectStore } from "./fixture-project-store";
import { taskFixtureOperations } from "./task-fixtures";

/**
 * DEV-only export handlers (`exportFixture: true` on the fixture marker), composed with the task
 * fixture handlers so Background tasks, Details and "Show in folder" read the same committed sample:
 *
 * - The capability report offers MP4 (H.264) and WebM; H.265 and ProRes are unavailable with reasons.
 * - A video render records a queued job and resolves on the third folder reload after it started
 *   (the store's job clock), which advances the job to running and then completed with a recorded
 *   render report. The editor reloads the folder about once a second while a task runs. While it
 *   runs, job progress reports reloads seen over reloads needed for each pending render.
 * - A render given an `output` records its export settings on the job and, on completion, an export
 *   artifact at the next free name in its folder (`Name.mp4`, `Name (2).mp4`, ...): `exports/...` for
 *   the project folder, or the absolute path for a chosen folder. The folder chooser answers
 *   `/tmp/video-creater-exports`.
 * - Premiere / DaVinci XML and the project package complete at once with a recorded export artifact.
 *
 * Reveals go through the task fixture's handler and land on `window.__EDITOR_FIXTURE_REVEALS__`.
 * Without `tasksFixture` the sample gets no seeded tasks, so the export is the only task.
 */

/** Folder reloads a render waits for before it resolves. */
export const exportFixtureRenderPolls = 3;

/** The folder the fixture's "Choose export folder" picks. */
const exportFixtureChosenFolder = "/tmp/video-creater-exports";

const h265Reason = "H.265 needs a hardware video encoder, and this computer doesn't report one.";
const proResReason = "ProRes needs the ProRes encoder, which this build doesn't include.";

interface ProfileShape {
  readonly profile: Exclude<ExportProfile, "palmierProject">;
  readonly label: string;
  readonly container: string;
  readonly extension: string;
  readonly mimeType: string;
  readonly videoCodec: string;
  readonly audioCodec: string;
  readonly requiredRuntime: string[];
  /** Why the profile can't export; null when it can. */
  readonly reason: string | null;
}

const profileShapes: readonly ProfileShape[] = [
  { profile: "webm", label: "WebM", container: "webm", extension: "webm", mimeType: "video/webm", videoCodec: "vp8", audioCodec: "opus", requiredRuntime: ["gstreamer:webmmux", "gstreamer:vp8enc", "gstreamer:opusenc"], reason: null },
  { profile: "mp4H264", label: "MP4 / H.264", container: "mp4", extension: "mp4", mimeType: "video/mp4", videoCodec: "h264", audioCodec: "aac", requiredRuntime: ["gstreamer:mp4mux", "gstreamer:openh264enc", "gstreamer:avenc_aac"], reason: null },
  { profile: "mp4H265", label: "MP4 / H.265", container: "mp4", extension: "mp4", mimeType: "video/mp4", videoCodec: "hevc", audioCodec: "aac", requiredRuntime: ["gstreamer:mp4mux", "gstreamer:vah265enc", "gstreamer:avenc_aac"], reason: h265Reason },
  { profile: "proResMov", label: "ProRes MOV", container: "mov", extension: "mov", mimeType: "video/quicktime", videoCodec: "prores", audioCodec: "pcm", requiredRuntime: ["gstreamer:qtmux", "gstreamer:avenc_prores_ks"], reason: proResReason },
];

/** The capability report in the native command's shape. */
function exportFixtureProfiles(): ExportProfileAvailability[] {
  return profileShapes.map(({ reason, ...shape }) => ({
    ...shape,
    requiredRuntime: [...shape.requiredRuntime],
    available: reason === null,
    policyStatus: reason === null ? "approved" : "missingRuntime",
    unavailableReason: reason,
    qualityAvailability: { draft: reason === null, final: reason === null },
    qualityUnavailableReasons: reason === null ? {} : { draft: reason, final: reason },
  }));
}

interface RenderInput {
  readonly projectDir: string;
  readonly projectId: string;
  readonly profile: ProfileShape["profile"];
  readonly quality: ExportQuality;
  readonly width: number;
  readonly height: number;
  readonly jobId: string;
  readonly attemptId: string;
  readonly updatedAt: string;
  readonly fps?: number;
  readonly encodeTier?: ExportEncodeTier;
  readonly output?: ExportOutput;
  readonly exportSettings?: JobExportSettings;
  readonly timelineId?: string;
  readonly rangeStartSeconds?: number;
  readonly rangeEndSeconds?: number;
}

/** A render waiting for folder reloads; `reject` settles it when the render is cancelled. */
interface PendingRender {
  readonly input: RenderInput;
  readonly sourceDuration: number;
  /** Admitted workers advance with time, independently of how many readers poll. */
  lastAdvanceAt?: number;
  polls: number;
  resolve(result: ProjectMediaRenderResult): void;
  reject(reason: unknown): void;
}

function stableInput(input: RenderInput): string {
  return JSON.stringify(Object.fromEntries(Object.entries(input).sort(([a], [b]) => a.localeCompare(b))));
}

function sourceDuration(project: VideoProject, input: RenderInput): number {
  const timeline = input.timelineId ? project.timelines?.find((entry) => entry.id === input.timelineId)?.timeline : project.timeline;
  if (!timeline) throw new Error("Requested timeline was not found");
  if (input.rangeStartSeconds === undefined && input.rangeEndSeconds === undefined) return timeline.durationSeconds;
  if (input.rangeStartSeconds === undefined || input.rangeEndSeconds === undefined || !Number.isFinite(input.rangeStartSeconds) || !Number.isFinite(input.rangeEndSeconds) || input.rangeEndSeconds <= input.rangeStartSeconds) throw new Error("Render range is invalid");
  return input.rangeEndSeconds - input.rangeStartSeconds;
}

function renderJob(input: RenderInput, status: ProjectJobSummary["status"], updatedAt: string): ProjectJobSummary {
  const { profile, quality, width, height, fps, encodeTier, output } = input;
  return {
    id: input.jobId,
    kind: "render_draft",
    status,
    updatedAt,
    workflow: { workflowId: `video-creater/${input.projectId}/${input.jobId}`, workflowType: "RenderDraftWorkflow", taskQueue: "video-creater-workflows", runId: input.attemptId, activityTypes: ["render_draft"] },
    ...(output ? { exportSettings: input.exportSettings ? { ...input.exportSettings, output } : { profile, quality, width, height, ...(fps === undefined ? {} : { fps }), ...(encodeTier === undefined ? {} : { encodeTier }), output } } : {}),
  };
}

/** The saved export's recorded path: the first free name in its folder, as the native exporter picks. */
function savedExportPath(project: VideoProject, input: RenderInput, output: ExportOutput): string {
  const shape = profileShapes.find((candidate) => candidate.profile === input.profile);
  const extension = shape?.extension ?? "mp4";
  const folder = output.directory ?? "exports";
  const stem = exportFileLabel(output.fileName, extension).slice(0, -(extension.length + 1));
  const taken = new Set((project.exportArtifacts ?? []).map((artifact) => artifact.path));
  for (let index = 1; ; index += 1) {
    const path = `${folder}/${index === 1 ? stem : `${stem} (${index.toString()})`}.${extension}`;
    if (!taken.has(path)) return path;
  }
}

function renderOutputPath(input: RenderInput): string {
  const extension = profileShapes.find((shape) => shape.profile === input.profile)?.extension ?? "mp4";
  return `renders/${input.jobId}/output.${extension}`;
}

function projectRenderReport(input: RenderInput, createdAt: string, durationSeconds: number): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id: input.jobId,
    status: "completed",
    outputPath: renderOutputPath(input),
    durationSeconds,
    streams: { video: true, audio: true },
    checks: { duration: "passed", streams: "passed", captionAlignment: "skipped" },
    artifacts: [`renders/${input.jobId}/pipeline-report.json`],
    previewComparisonRequest: null,
    previewComparison: null,
    logPath: `renders/${input.jobId}/render.log`,
    createdAt,
  };
}

function pipelineReport(input: RenderInput, durationSeconds: number): RenderReport {
  const sample = buildSampleRenderReport(input.quality === "draft" ? "draftWebm" : "finalWebm");
  const shape = profileShapes.find((candidate) => candidate.profile === input.profile);
  return {
    ...sample,
    jobId: input.jobId,
    summary: {
      ...sample.summary,
      status: "ready",
      durationSeconds,
      outputPath: renderOutputPath(input),
      requestedWidth: input.width,
      requestedHeight: input.height,
      actualWidth: input.width,
      actualHeight: input.height,
      quality: input.quality,
      ...(shape ? { container: shape.container, videoCodec: shape.videoCodec, audioCodec: shape.audioCodec } : {}),
    },
  };
}

function upsertJob(project: VideoProject, job: ProjectJobSummary): VideoProject {
  const exists = project.jobs.some((candidate) => candidate.id === job.id);
  return { ...project, jobs: exists ? project.jobs.map((candidate) => (candidate.id === job.id ? job : candidate)) : [...project.jobs, job] };
}

function exportArtifact(jobId: string, kind: ProjectExportArtifact["kind"], format: string, path: string, mimeType: string, createdAt: string): ProjectExportArtifact {
  return { schemaVersion: 1, id: `artifact-${jobId}`, kind, format, path, mimeType, jobId, createdAt };
}

function startRequest(projectId: string, projectDir: string, jobId: string, workflowType: string, input: Record<string, unknown>): NonNullable<ProjectJobSummary["startRequest"]> {
  return {
    workflowId: `video-creater/${projectId}/${jobId}`,
    workflowType,
    taskQueue: "video-creater-workflows",
    input: { projectId, projectDir, jobId, ...input },
    searchAttributes: { projectId, jobId },
    activityTypes: [],
    idReusePolicy: "rejectDuplicate",
  };
}

function absolutePath(projectDir: string, relativePath: string): string {
  return `${projectDir.replace(/[\\/]+$/, "")}/${relativePath}`;
}

/** `seedTasks` also seeds the task fixture's tasks into the sample on its first save. */
export function exportFixtureOperations(store: FixtureProjectStore, options: { readonly seedTasks?: boolean } = {}): ReadonlyMap<string, FixtureOperationHandler> {
  const pending = new Map<string, PendingRender>();
  const attempts = new Map<string, { input: RenderInput; outcome: MediaRenderAttempt; sourceRevision: number }>();
  const attemptKey = (jobId: string, attemptId: string) => JSON.stringify([jobId, attemptId]);
  function requireProject(): VideoProject {
    if (!store.current) throw new Error("Open the sample project before exporting.");
    return store.current;
  }
  const record = (project: VideoProject) => store.record(project);

  /** One folder reload: each waiting render runs, then completes on its third reload. */
  function advance(): void {
    for (const render of [...pending.values()]) {
      const canonical = requireProject().jobs.find((job) => job.id === render.input.jobId);
      if (canonical?.workflow?.runId !== render.input.attemptId || ["cancelled", "failed", "completed"].includes(canonical.status)) {
        pending.delete(render.input.jobId);
        render.reject("The render attempt was superseded or stopped.");
        continue;
      }
      if (render.lastAdvanceAt !== undefined) {
        if (Date.now() - render.lastAdvanceAt < 1_000) continue;
        render.lastAdvanceAt = Date.now();
      }
      render.polls += 1;
      const now = new Date().toISOString();
      if (render.polls < exportFixtureRenderPolls) {
        record(upsertJob(requireProject(), renderJob(render.input, "running", now)));
        continue;
      }
      pending.delete(render.input.jobId);
      const base = requireProject();
      const duration = render.sourceDuration;
      const report = projectRenderReport(render.input, now, duration);
      const shape = profileShapes.find((candidate) => candidate.profile === render.input.profile);
      const output = render.input.output;
      const saved = output
        ? exportArtifact(render.input.jobId, render.input.profile === "webm" ? "webm" : "mp4", render.input.profile, savedExportPath(base, render.input, output), shape?.mimeType ?? "video/mp4", now)
        : null;
      const project = record({
        ...upsertJob(base, renderJob(render.input, "completed", now)),
        renderReports: [...base.renderReports.filter((candidate) => candidate.id !== report.id), report],
        ...(saved ? { exportArtifacts: [...(base.exportArtifacts ?? []), saved] } : {}),
      });
      render.resolve({ project, renderReport: pipelineReport(render.input, duration), projectRenderReport: report, outputPath: report.outputPath, ...(saved ? { exportArtifact: saved } : {}) });
    }
  }
  store.onReload(advance);

  return new Map<string, FixtureOperationHandler>([
    ...taskFixtureOperations(store, { seed: options.seedTasks === true }),
    ["get_export_profile_availability_report", () => exportFixtureProfiles()],
    ["open_export_directory_dialog", () => exportFixtureChosenFolder],
    [
      "load_job_progress_from_split_project_folder",
      (): JobProgressSnapshot[] =>
        // A render reports progress once it runs, after its first reload.
        [...pending.values()]
          .filter((render) => render.polls > 0)
          .map((render) => ({ jobId: render.input.jobId, progress: render.polls / exportFixtureRenderPolls, updatedAt: new Date().toISOString() })),
    ],
    [
      "render_media_to_split_project_folder",
      (input) => {
        const request = input as unknown as RenderInput;
        const project = requireProject();
        sourceDuration(project, request);
        if (input.admissionProtocol !== undefined) {
          if (input.admissionProtocol !== 1) throw new Error("Unsupported render admission protocol");
          const key = attemptKey(request.jobId, request.attemptId);
          const existing = attempts.get(key);
          if (existing) {
            if (stableInput(existing.input) !== stableInput(request)) throw new Error("Render attempt identity was reused with different inputs");
            return { admissionProtocol: 1, project, jobId: request.jobId, attemptId: request.attemptId, sourceRevision: existing.sourceRevision };
          }
          if (input.expectedRevision !== project.contentRevision) throw new Error("Project revision conflict");
          if (request.projectId !== project.id) throw new Error("Render project identity does not match");
          if (pending.has(request.jobId)) throw new Error("Render job is busy");
          const record = { input: request, outcome: { status: "pending" } as MediaRenderAttempt, sourceRevision: project.contentRevision ?? 0 };
          attempts.set(key, record);
          const admitted = recordProject(request);
          pending.set(request.jobId, { input: request, sourceDuration: sourceDuration(project, request), lastAdvanceAt: Date.now(), polls: 0, resolve: (result) => { record.outcome = { status: "completed", result }; }, reject: () => { record.outcome = { status: "failed", message: "The render was cancelled." }; } });
          return { admissionProtocol: 1, project: admitted, jobId: request.jobId, attemptId: request.attemptId, sourceRevision: record.sourceRevision };
        }
        if (pending.has(request.jobId)) throw new Error(`A render for ${request.jobId} is already running.`);
        record(upsertJob(project, renderJob(request, "queued", request.updatedAt)));
        return new Promise<ProjectMediaRenderResult>((resolve, reject) => {
          pending.set(request.jobId, { input: request, sourceDuration: sourceDuration(project, request), polls: 0, resolve, reject });
        });
      },
    ],
    ["recover_render_attempt_in_split_project_folder", (input) => {
      const attempt = attempts.get(attemptKey(String(input.jobId), String(input.attemptId)));
      if (!attempt) throw new Error("Render attempt was not found");
      return attempt.outcome;
    }],
    ["load_render_attempt_in_split_project_folder", (input) => {
      const record = attempts.get(attemptKey(String(input.jobId), String(input.attemptId)));
      if (!record) throw new Error("Render attempt was not found");
      advance();
      return record.outcome.status === "completed" ? { status: "completed", result: { ...record.outcome.result, project: requireProject() } } : record.outcome;
    }],
    [
      "cancel_render_job_in_split_project_folder",
      (input) => {
        const { jobId, updatedAt } = input as { jobId: string; updatedAt: string };
        const render = pending.get(jobId);
        if (!render) throw new Error("This render is no longer running.");
        if (input.attemptId !== render.input.attemptId) throw new Error("Render attempt identity does not match");
        pending.delete(jobId);
        const project = record(upsertJob(requireProject(), renderJob(render.input, "cancelled", updatedAt)));
        render.reject("The render was cancelled.");
        const result: ProjectActionWriteResult = { project, report: fixtureWriteReport() };
        return result;
      },
    ],
    [
      "export_nle_xml_to_split_project_folder",
      (input) => {
        const { projectDir, format, jobId, updatedAt } = input as { projectDir: string; format: NleXmlExportFormat; jobId: string; updatedAt: string };
        const base = requireProject();
        const filename = format === "premiereXmeml" ? `${base.id}-premiere.xml` : `${base.id}-davinci.fcpxml`;
        const path = `exports/${filename}`;
        const job: ProjectJobSummary = {
          id: jobId,
          kind: "export_nle_xml",
          status: "completed",
          updatedAt,
          startRequest: startRequest(base.id, projectDir, jobId, "ExportNleXmlWorkflow", { format, outputPath: path }),
        };
        const project = record({ ...upsertJob(base, job), exportArtifacts: [...(base.exportArtifacts ?? []), exportArtifact(jobId, "nle_xml", format, path, "application/xml", updatedAt)] });
        const result: NleXmlExportCommandResult = { project, exportPath: absolutePath(projectDir, path), job };
        return result;
      },
    ],
    [
      "export_palmier_project_package_to_split_project_folder",
      (input) => {
        const { projectDir, jobId, outputPath, updatedAt } = input as { projectDir: string; jobId: string; outputPath: string; updatedAt: string };
        const base = requireProject();
        const path = outputPath || mediaExportOutputPath(base.id, "palmierProject", "palmier", jobId);
        const job: ProjectJobSummary = {
          id: jobId,
          kind: "export_media",
          status: "completed",
          updatedAt,
          startRequest: startRequest(base.id, projectDir, jobId, "ExportProjectBundleWorkflow", { profile: "palmierProject", outputPath: path }),
        };
        const project = record({
          ...upsertJob(base, job),
          exportArtifacts: [...(base.exportArtifacts ?? []), exportArtifact(jobId, "project_bundle", "palmierProject", path, "application/zip", updatedAt)],
        });
        const result: NleXmlExportCommandResult = { project, exportPath: absolutePath(projectDir, path), job };
        return result;
      },
    ],
  ]);
  function recordProject(request: RenderInput): VideoProject {
    return store.write(upsertJob(requireProject(), renderJob(request, "queued", request.updatedAt)));
  }
}
