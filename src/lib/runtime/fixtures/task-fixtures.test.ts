import { describe, expect, it, vi } from "vitest";
import { indicatorState } from "@/lib/jobs/task-indicator";
import { taskRecords, temporalCancelReason } from "@/lib/jobs/task-records";
import type { JobProgressSnapshot, ProjectPreviewRenderComparisonRunResult, TemporalJobReconciliation, TemporalWorkerEnvironmentReport, VideoProject } from "@/lib/project";
import type { RenderReport } from "@/lib/render";
import { createSampleProject } from "@/lib/sample-project";
import { createFixtureProjectStore } from "./fixture-project-store";
import { projectFixtureOperations } from "./project-fixtures";
import { taskFixtureOperations } from "./task-fixtures";

const projectDir = "/tmp/video-creater-editor-project";

function operations(seed?: (project: VideoProject) => VideoProject) {
  const store = createFixtureProjectStore();
  if (seed) store.addSeed(seed);
  const handlers = new Map([...projectFixtureOperations(store), ...taskFixtureOperations(store)]);
  return async function request<Result>(operation: string, input: Record<string, unknown> = {}): Promise<Result> {
    const handler = handlers.get(operation);
    if (!handler) throw new Error(`no task fixture handler for ${operation}`);
    return (await handler(input)) as Result;
  };
}

function records(project: VideoProject) {
  return taskRecords(project, { projectDir, executionBackend: "inProcess", activeRender: null, agentTurn: null, progressByJobId: new Map(), workflowServiceIssue: null, exportPlanJobIds: new Set() });
}

describe("task fixture operations", () => {
  it("seeds a running Temporal transcription, a completed export and a failed render into the sample once", async () => {
    const request = operations();
    const { project } = await request<{ project: VideoProject }>("save_split_project_to_folder", { projectDir, project: createSampleProject() });
    const tasks = records(project);
    expect(tasks.map((task) => [task.kind, task.status])).toEqual(
      expect.arrayContaining([
        ["transcription", "running"],
        ["export", "completed"],
        ["render", "failed"],
        ["export", "queued"],
      ]),
    );
    const stale = project.jobs.find((job) => job.id === "fixture-export-nle-stale");
    expect(stale).toMatchObject({ kind: "export_nle_xml", status: "queued", startRequest: { input: { format: "davinciFcpxml" } } });
    expect(stale?.workflow?.runId ?? null).toBeNull();
    expect(Date.now() - Date.parse(stale?.updatedAt ?? "")).toBeGreaterThanOrEqual(9 * 60_000);
    const transcription = tasks.find((task) => task.kind === "transcription");
    expect(transcription?.cancel).toEqual({ available: false, reason: temporalCancelReason });
    expect(indicatorState(tasks, Date.now())).toMatchObject({ visible: true, tone: "running", label: "Transcribing…" });
    expect(project.renderReports.find((report) => report.id === "fixture-export-mp4")?.previewComparisonRequest?.projectDir).toBe(projectDir);

    const again = await request<{ project: VideoProject }>("save_split_project_to_folder", { projectDir, project });
    expect(again.project.jobs).toHaveLength(project.jobs.length);
  });

  it("records reveal requests for recorded export paths and refuses anything else", async () => {
    delete window.__EDITOR_FIXTURE_REVEALS__;
    const request = operations();
    await request("save_split_project_to_folder", { projectDir, project: createSampleProject() });

    await request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: "renders/fixture-export-mp4/output.mp4" });
    await request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: "renders/fixture-export-mp4/render.log" });
    await expect(request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: "media/input.mp4" })).rejects.toBe(
      "This file isn't a recorded export of this project.",
    );
    expect(window.__EDITOR_FIXTURE_REVEALS__).toEqual([
      { projectDir, artifactPath: "renders/fixture-export-mp4/output.mp4" },
      { projectDir, artifactPath: "renders/fixture-export-mp4/render.log" },
    ]);
    delete window.__EDITOR_FIXTURE_REVEALS__;
  });

  it("seeds on top of earlier seeds and leaves other projects alone", async () => {
    const committed = vi.fn((project: VideoProject) => ({ ...project, name: "Committed" }));
    const request = operations(committed);
    const sample = await request<{ project: VideoProject }>("save_split_project_to_folder", { projectDir, project: createSampleProject() });
    expect(sample.project.name).toBe("Committed");
    expect(sample.project.jobs.some((job) => job.id === "fixture-render-draft")).toBe(true);

    const other = { ...createSampleProject(), id: "project-other" };
    expect((await operations()<{ project: VideoProject }>("save_split_project_to_folder", { project: other })).project.jobs).toEqual(other.jobs);
  });

  it("answers the worker preflight, the failed render's pipeline report and a preview comparison", async () => {
    const request = operations();
    const { project } = await request<{ project: VideoProject }>("save_split_project_to_folder", { projectDir, project: createSampleProject() });
    const worker = await request<TemporalWorkerEnvironmentReport>("get_temporal_worker_environment_report");
    expect(worker.tools.some((tool) => !tool.available)).toBe(true);
    const pipeline = await request<RenderReport>("load_render_pipeline_report_from_split_project_folder", { projectDir, jobId: "fixture-render-draft" });
    expect(pipeline.summary.status).toBe("failed");
    expect(pipeline.errors).toHaveLength(1);

    const comparisonRequest = project.renderReports.find((report) => report.id === "fixture-export-mp4")?.previewComparisonRequest;
    const result = await request<ProjectPreviewRenderComparisonRunResult>("run_preview_render_comparison_request_in_split_project_folder", {
      projectDir,
      request: comparisonRequest,
      updatedAt: "2026-09-15T10:00:00Z",
    });
    expect(result.renderReport.previewComparison?.comparedFrames).toHaveLength(2);
    expect(result.project.renderReports.find((report) => report.id === "fixture-export-mp4")?.previewComparison?.status).toBe("failed");
  });

  it("reconciles the stale DaVinci XML export as never started and reports no progress", async () => {
    const request = operations();
    await request("save_split_project_to_folder", { projectDir, project: createSampleProject() });

    await expect(request<JobProgressSnapshot[]>("load_job_progress_from_split_project_folder", { projectDir })).resolves.toEqual([]);
    const result = await request<TemporalJobReconciliation>("reconcile_temporal_jobs_in_split_project_folder", { projectDir, updatedAt: new Date().toISOString() });

    expect(result).toMatchObject({ failedJobIds: ["fixture-export-nle-stale"], serviceReachable: true, detail: null });
    const stale = result.project?.jobs.find((job) => job.id === "fixture-export-nle-stale");
    expect(stale).toMatchObject({ status: "failed", failureReason: "The workflow never started." });
    expect(result.project?.jobs.find((job) => job.id === "fixture-transcribe-media-1")?.status).toBe("running");
    const task = records(result.project as VideoProject).find((record) => record.id === "fixture-export-nle-stale");
    expect(task).toMatchObject({ label: "DaVinci XML export", status: "failed", failureReason: "The workflow never started.", retry: true });

    const again = await request<TemporalJobReconciliation>("reconcile_temporal_jobs_in_split_project_folder", { projectDir, updatedAt: new Date().toISOString() });
    expect(again.failedJobIds).toEqual([]);
  });
});
