import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ProjectJobSummary,
  ProjectPreviewRenderComparisonRunResult,
  ProjectRenderReport,
  TemporalWorkerEnvironmentReport,
  VideoProject,
} from "@/lib/project";
import { buildSampleRenderReport, type RenderPreviewComparison, type RenderPreviewComparisonRequest, type RenderReport } from "@/lib/render";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { TaskDetailsDialog } from "./task-details-dialog";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

const projectDir = "/projects/demo";
const updatedAt = "2026-09-15T10:00:00Z";

const preflightReport: TemporalWorkerEnvironmentReport = {
  ready: false,
  featureEnabled: true,
  taskQueue: "video-creater-workflows",
  localServiceTarget: "127.0.0.1:7233",
  localWebUiUrl: "http://127.0.0.1:8233",
  localDevCommand: "temporal server start-dev",
  workerRunCommand: "cargo run --bin video-creater-worker",
  featureName: "temporal-worker",
  tools: [
    { name: "temporal", available: true, path: "/usr/local/bin/temporal" },
    { name: "ffmpeg", available: false, installHint: "Install ffmpeg with your package manager." },
  ],
};

const comparisonRequest: RenderPreviewComparisonRequest = {
  status: "pending",
  projectDir,
  projectReportId: "export-1",
  renderReportPath: "renders/export-1/pipeline-report.json",
  renderedVideo: "renders/export-1/output.mp4",
  durationSeconds: 4,
  frameTimeSeconds: 1,
  renderedFrames: ["renders/export-1/frames/frame-0.png"],
};

const mismatch: RenderPreviewComparison = {
  status: "failed",
  comparedFrames: [
    { timelineSeconds: 0, previewFrame: "preview/0.png", renderedFrame: "rendered/0.png", diffFrame: null, mismatchRatio: 0, passed: true },
    { timelineSeconds: 1.5, previewFrame: "preview/1.png", renderedFrame: "rendered/1.png", diffFrame: "diff/1.png", mismatchRatio: 0.083333, passed: false },
  ],
};

function exportReport(overrides: Partial<ProjectRenderReport> = {}): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id: "export-1",
    status: "completed",
    outputPath: "renders/export-1/output.mp4",
    durationSeconds: 4,
    streams: { video: true, audio: false },
    checks: { duration: "passed", captionAlignment: "failed" },
    artifacts: ["renders/export-1/pipeline-report.json"],
    previewComparisonRequest: comparisonRequest,
    previewComparison: null,
    logPath: "renders/export-1/render.log",
    createdAt: updatedAt,
    ...overrides,
  };
}

function project(jobs: readonly ProjectJobSummary[], renderReports: readonly ProjectRenderReport[] = []): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, jobs: [...jobs], generatedAssets: [], renderReports: [...renderReports], exportArtifacts: [] };
}

const temporalJob: ProjectJobSummary = {
  id: "transcribe-1",
  kind: "transcribe_media",
  status: "running",
  updatedAt,
  workflow: { workflowId: "video-creater/demo/transcribe-1", workflowType: "TranscribeMedia", taskQueue: "video-creater-workflows", runId: "run-42", activityTypes: [] },
};

function setup(currentProject: VideoProject, taskId: string, handlers: Record<string, Handler> = {}) {
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = handlers[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
  const result = renderWithEditorStore(<TaskDetailsDialog />, { project: currentProject, projectDir });
  act(() => result.store.getState().openTaskDetails(taskId));
  return result;
}

function calls(command: string) {
  return vi.mocked(backendRequest).mock.calls.filter(([name]) => name === command);
}

describe("TaskDetailsDialog", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
  });

  it("shows workflow metadata and the worker preflight report for a Temporal task", async () => {
    const { store } = setup(project([temporalJob]), "transcribe-1", { get_temporal_worker_environment_report: () => preflightReport });
    const dialog = await screen.findByRole("dialog", { name: "Task details" });
    const workflow = within(dialog).getByRole("region", { name: "Workflow" });
    expect(within(workflow).getByText("Temporal workflow worker")).toBeInTheDocument();
    expect(within(workflow).getByText("video-creater/demo/transcribe-1")).toBeInTheDocument();
    expect(within(workflow).getByText("run-42")).toBeInTheDocument();
    expect(within(workflow).getByText("video-creater-workflows")).toBeInTheDocument();
    expect(dialog.querySelector(`time[datetime="${updatedAt}"]`)).not.toBeNull();

    expect(await within(dialog).findByText("Setup needed")).toBeInTheDocument();
    const preflight = within(dialog).getByRole("region", { name: "Worker preflight" });
    expect(within(preflight).getByText("ffmpeg")).toBeInTheDocument();
    expect(within(preflight).getByText("Install ffmpeg with your package manager.")).toBeInTheDocument();
    expect(within(preflight).getByText("127.0.0.1:7233")).toBeInTheDocument();
    expect(calls("get_temporal_worker_environment_report")).toHaveLength(1);

    fireEvent.click(within(dialog).getByRole("button", { name: "Close Task details" }));
    expect(store.getState().taskDetailsId).toBeNull();
  });

  it("explains why the workflow service can't be used for an active Temporal task", async () => {
    const { store } = setup(project([temporalJob]), "transcribe-1", { get_temporal_worker_environment_report: () => preflightReport });
    act(() => store.setState({ workflowServiceIssue: 'Workflow namespace "video-creater" not found' }));
    const dialog = await screen.findByRole("dialog", { name: "Task details" });
    const status = within(dialog).getByRole("region", { name: "Status" });
    expect(within(status).getByText("Workflow service")).toBeInTheDocument();
    expect(within(status).getByText('Workflow namespace "video-creater" not found')).toBeInTheDocument();

    act(() => store.setState({ workflowServiceIssue: null }));
    expect(within(status).queryByText("Workflow service")).not.toBeInTheDocument();
  });

  it("does not check the worker for in-process tasks", async () => {
    setup(project([{ id: "render-1", kind: "render_draft", status: "failed", updatedAt }]), "render-1", {
      get_temporal_worker_environment_report: () => preflightReport,
      load_render_pipeline_report_from_split_project_folder: () => buildSampleRenderReport("draftWebm"),
    });
    const dialog = await screen.findByRole("dialog", { name: "Task details" });
    expect(within(dialog).getByText("Editor process")).toBeInTheDocument();
    expect(within(dialog).queryByRole("region", { name: "Worker preflight" })).not.toBeInTheDocument();
    expect(calls("get_temporal_worker_environment_report")).toHaveLength(0);
  });

  it("renders the recorded render report metrics without loading the pipeline report", async () => {
    setup(project([{ id: "export-1", kind: "export_media", status: "completed", updatedAt }], [exportReport({ previewComparison: mismatch })]), "export-1");
    const dialog = await screen.findByRole("dialog", { name: "Task details" });
    const metrics = within(dialog).getByRole("region", { name: "Render report" });
    expect(within(metrics).getByText("renders/export-1/output.mp4")).toBeInTheDocument();
    expect(within(metrics).getByText("4s")).toBeInTheDocument();
    expect(within(metrics).getByText("Missing")).toBeInTheDocument();
    expect(within(metrics).getByText("Caption alignment")).toBeInTheDocument();
    expect(within(metrics).getByText("1 of 2 frames match")).toBeInTheDocument();
    expect(within(metrics).getByText("8.33% diff")).toBeInTheDocument();
    expect(calls("load_render_pipeline_report_from_split_project_folder")).toHaveLength(0);
  });

  it("loads the pipeline report when the project has no render report for the task", async () => {
    const sample = buildSampleRenderReport("finalWebm");
    const pipeline: RenderReport = {
      ...sample,
      jobId: "render-1",
      summary: { ...sample.summary, status: "failed", outputPath: "renders/render-1/output.mp4", quality: "final", container: "mp4", actualWidth: 1920, actualHeight: 1080 },
      errors: [{ code: "font_missing", path: "timeline.items[3]", message: "The font “Canela” is missing.", fix: "Install the font or pick another one." }],
    };
    setup(project([{ id: "render-1", kind: "render_draft", status: "failed", updatedAt }]), "render-1", {
      load_render_pipeline_report_from_split_project_folder: () => pipeline,
    });
    const dialog = await screen.findByRole("dialog", { name: "Task details" });
    const metrics = await within(dialog).findByRole("region", { name: "Render report" });
    expect(within(metrics).getByText("Final MP4")).toBeInTheDocument();
    expect(within(metrics).getByText("1920 × 1080")).toBeInTheDocument();
    expect(within(metrics).getByText("The font “Canela” is missing.")).toBeInTheDocument();
    expect(within(metrics).getByText("Install the font or pick another one.")).toBeInTheDocument();
    expect(calls("load_render_pipeline_report_from_split_project_folder")[0]?.[1]).toEqual({ projectDir, jobId: "render-1" });
  });

  it("Compare preview and render runs the comparison request and shows the result", async () => {
    const recorded = exportReport();
    const initial = project([{ id: "export-1", kind: "export_media", status: "completed", updatedAt }], [recorded]);
    const compared = exportReport({ previewComparison: mismatch, previewComparisonRequest: { ...comparisonRequest, status: "completed" } });
    const pipeline = { ...buildSampleRenderReport("finalWebm"), jobId: "export-1", previewComparison: mismatch };
    const result: ProjectPreviewRenderComparisonRunResult = {
      project: { ...initial, renderReports: [compared] },
      renderReport: pipeline,
      projectRenderReport: compared,
      evidenceReport: "renders/export-1/preview-comparison.json",
    };
    let resolveRun: (value: ProjectPreviewRenderComparisonRunResult) => void = () => undefined;
    const { store } = setup(initial, "export-1", {
      run_preview_render_comparison_request_in_split_project_folder: () => new Promise((resolve) => (resolveRun = resolve)),
    });
    const dialog = await screen.findByRole("dialog", { name: "Task details" });
    const compare = within(dialog).getByRole("button", { name: "Compare preview and render" });
    fireEvent.click(compare);
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Comparing…" })).toBeDisabled());

    const [call] = calls("run_preview_render_comparison_request_in_split_project_folder");
    expect(call?.[1]).toEqual(expect.objectContaining({ projectDir, request: { ...comparisonRequest, status: "pending" }, updatedAt: expect.any(String) }));

    await act(async () => resolveRun(result));
    expect(await within(dialog).findByText("Preview and render differ on 1 of 2 frames.")).toBeInTheDocument();
    expect(store.getState().project.renderReports[0]?.previewComparison).toEqual(mismatch);
  });

  it("explains a comparison that couldn't run", async () => {
    setup(project([{ id: "export-1", kind: "export_media", status: "completed", updatedAt }], [exportReport()]), "export-1", {
      run_preview_render_comparison_request_in_split_project_folder: () => {
        throw new Error("The rendered video is missing.");
      },
    });
    const dialog = await screen.findByRole("dialog", { name: "Task details" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Compare preview and render" }));
    expect(await within(dialog).findByText("The comparison couldn't run. The rendered video is missing.")).toBeInTheDocument();
  });
});
