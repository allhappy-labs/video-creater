import { afterEach, describe, expect, it } from "vitest";
import { exportChoiceOptions, defaultExportChoices } from "@/lib/export/export-plan";
import { taskRecords } from "@/lib/jobs/task-records";
import type { ExportProfileAvailability, JobProgressSnapshot, NleXmlExportCommandResult, ProjectMediaRenderResult, VideoProject } from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { exportFixtureOperations, exportFixtureRenderPolls } from "./export-fixtures";
import { createFixtureProjectStore } from "./fixture-project-store";
import { projectFixtureOperations } from "./project-fixtures";

const projectDir = "/tmp/video-creater-editor-project";

function operations(options: { readonly seedTasks?: boolean } = {}) {
  const store = createFixtureProjectStore();
  const handlers = new Map([...projectFixtureOperations(store), ...exportFixtureOperations(store, options)]);
  return async function request<Result>(operation: string, input: Record<string, unknown> = {}): Promise<Result> {
    const handler = handlers.get(operation);
    if (!handler) throw new Error(`no export fixture handler for ${operation}`);
    return (await handler(input)) as Result;
  };
}

async function openSample(request: ReturnType<typeof operations>): Promise<VideoProject> {
  return (await request<{ project: VideoProject }>("save_split_project_to_folder", { projectDir, project: createSampleProject() })).project;
}

function renderInput(jobId: string) {
  return { projectDir, projectId: "project-sample", profile: "mp4H264", quality: "final", width: 1920, height: 1080, jobId, attemptId: "render-attempt/1", updatedAt: "2026-09-15T10:00:00Z" };
}

function records(project: VideoProject, exportJobIds: readonly string[] = []) {
  return taskRecords(project, { projectDir, executionBackend: "inProcess", activeRender: null, agentTurn: null, progressByJobId: new Map(), workflowServiceIssue: null, exportPlanJobIds: new Set(exportJobIds) });
}

/** Resolves true once the promise settled, without waiting for it. */
async function settled(promise: Promise<unknown>): Promise<boolean> {
  let done = false;
  void promise.then(
    () => (done = true),
    () => (done = true),
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
  return done;
}

describe("export fixture operations", () => {
  afterEach(() => {
    delete window.__EDITOR_FIXTURE_REVEALS__;
  });

  it("offers MP4 and WebM, with H.265 and ProRes unavailable and saying why", async () => {
    const request = operations();
    const report = await request<ExportProfileAvailability[]>("get_export_profile_availability_report");
    expect(report.map((profile) => [profile.profile, profile.available])).toEqual([
      ["webm", true],
      ["mp4H264", true],
      ["mp4H265", false],
      ["proResMov", false],
    ]);
    const choices = defaultExportChoices(report, "Sample");
    expect(choices).toMatchObject({ format: "mp4", codec: "h264", resolution: "1080p", quality: "high" });
    const options = exportChoiceOptions(report, choices);
    expect(options.format.find((option) => option.value === "prores")?.disabledReason).toMatch(/ProRes/);
    expect(options.codec.find((option) => option.value === "h265")?.disabledReason).toMatch(/H\.265/);
  });

  it("opens the sample as a folder project without seeded tasks unless asked", async () => {
    const plain = operations();
    const project = await openSample(plain);
    expect(project.schemaVersion).toBe(2);
    expect(project.jobs).toEqual(createSampleProject().jobs);

    const seeded = await openSample(operations({ seedTasks: true }));
    expect(seeded.jobs.some((job) => job.id === "fixture-transcribe-media-1")).toBe(true);
  });

  it("resolves a render on the third folder reload with a recorded render report that Show in folder reveals", async () => {
    const request = operations();
    await expect(request("render_media_to_split_project_folder", renderInput("export-mp4H264-a"))).rejects.toThrow("Open the sample project");
    await expect(request("load_split_project_from_folder", { projectDir })).rejects.toThrow();
    await openSample(request);

    const render = request<ProjectMediaRenderResult>("render_media_to_split_project_folder", renderInput("export-mp4H264-a"));
    const statuses: string[] = [];
    const progress: JobProgressSnapshot[][] = [];
    for (let poll = 1; poll <= exportFixtureRenderPolls; poll += 1) {
      expect(await settled(render)).toBe(false);
      const loaded = await request<VideoProject>("load_split_project_from_folder", { projectDir });
      statuses.push(loaded.jobs.find((job) => job.id === "export-mp4H264-a")?.status ?? "missing");
      progress.push(await request<JobProgressSnapshot[]>("load_job_progress_from_split_project_folder", { projectDir }));
    }
    expect(statuses).toEqual(["running", "running", "completed"]);
    expect(progress.map((snapshots) => snapshots.map(({ jobId, progress: value }) => [jobId, value]))).toEqual([
      [["export-mp4H264-a", 1 / exportFixtureRenderPolls]],
      [["export-mp4H264-a", 2 / exportFixtureRenderPolls]],
      [],
    ]);

    const result = await render;
    expect(result.outputPath).toBe("renders/export-mp4H264-a/output.mp4");
    expect(result.projectRenderReport).toMatchObject({ id: "export-mp4H264-a", status: "completed", logPath: "renders/export-mp4H264-a/render.log" });
    expect(result.renderReport.summary).toMatchObject({ outputPath: result.outputPath, videoCodec: "h264", actualWidth: 1920 });
    const [task] = records(result.project, ["export-mp4H264-a"]);
    expect(task).toMatchObject({ kind: "export", status: "completed", label: "Video export", artifactPath: result.outputPath, workflow: { backend: "inProcess" } });

    await request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: result.outputPath });
    await request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: `${projectDir}/${result.projectRenderReport.logPath}` });
    await expect(request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: "/etc/passwd" })).rejects.toBe("This file isn't a recorded export of this project.");
    expect(window.__EDITOR_FIXTURE_REVEALS__?.map((reveal) => reveal.artifactPath)).toEqual([result.outputPath, `${projectDir}/renders/export-mp4H264-a/render.log`]);
  });

  /** Starts a render and reloads the folder until it resolves. */
  async function renderToCompletion(request: ReturnType<typeof operations>, input: Record<string, unknown>): Promise<ProjectMediaRenderResult> {
    const render = request<ProjectMediaRenderResult>("render_media_to_split_project_folder", input);
    for (let poll = 1; poll <= exportFixtureRenderPolls; poll += 1) await request("load_split_project_from_folder", { projectDir });
    return render;
  }

  it("saves a named export into the project's exports folder, taking the next free name", async () => {
    const request = operations();
    await openSample(request);
    const output = { fileName: "Edison intro", directory: null };

    const first = await renderToCompletion(request, { ...renderInput("export-mp4H264-named-1"), output });
    expect(first.exportArtifact?.path).toBe("exports/Edison intro.mp4");
    expect(first.project.exportArtifacts?.map((artifact) => artifact.path)).toContain("exports/Edison intro.mp4");
    expect(first.project.jobs.find((job) => job.id === "export-mp4H264-named-1")?.exportSettings).toEqual({ profile: "mp4H264", quality: "final", width: 1920, height: 1080, output });

    const second = await renderToCompletion(request, { ...renderInput("export-mp4H264-named-2"), attemptId: "render-attempt/2", output });
    expect(second.exportArtifact?.path).toBe("exports/Edison intro (2).mp4");
  });

  it("saves into a chosen folder by absolute path and reveals it", async () => {
    const request = operations();
    await openSample(request);
    await expect(request("open_export_directory_dialog", { defaultPath: `${projectDir}/exports` })).resolves.toBe("/tmp/video-creater-exports");

    const result = await renderToCompletion(request, {
      ...renderInput("export-mp4H264-chosen"),
      fps: 25,
      encodeTier: "master",
      output: { fileName: "Edison intro", directory: "/tmp/video-creater-exports" },
    });
    const path = "/tmp/video-creater-exports/Edison intro.mp4";
    expect(result.exportArtifact?.path).toBe(path);
    expect(result.project.jobs.find((job) => job.id === "export-mp4H264-chosen")?.exportSettings).toMatchObject({ fps: 25, encodeTier: "master" });
    await request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: path });
    expect(window.__EDITOR_FIXTURE_REVEALS__?.map((reveal) => reveal.artifactPath)).toEqual([path]);
  });

  it("records no export artifact for a render without an output (save range)", async () => {
    const request = operations();
    await openSample(request);
    const result = await renderToCompletion(request, { ...renderInput("save-range-a"), profile: "webm", quality: "draft" });
    expect(result.exportArtifact).toBeUndefined();
    expect(result.project.exportArtifacts ?? []).toEqual([]);
  });

  it("cancels a waiting render, leaving its job cancelled", async () => {
    const request = operations();
    await openSample(request);
    const render = request("render_media_to_split_project_folder", renderInput("export-mp4H264-b"));
    await request("load_split_project_from_folder", { projectDir });
    const { project } = await request<{ project: VideoProject }>("cancel_render_job_in_split_project_folder", { projectDir, jobId: "export-mp4H264-b", attemptId: "render-attempt/1", updatedAt: "2026-09-15T10:00:02Z" });
    await expect(render).rejects.toBe("The render was cancelled.");
    expect(project.jobs.find((job) => job.id === "export-mp4H264-b")?.status).toBe("cancelled");
    const loaded = await request<VideoProject>("load_split_project_from_folder", { projectDir });
    expect(loaded.renderReports.some((report) => report.id === "export-mp4H264-b")).toBe(false);
  });

  it("records XML and project package exports as completed tasks with revealable artifacts", async () => {
    const request = operations();
    await openSample(request);
    const premiere = await request<NleXmlExportCommandResult>("export_nle_xml_to_split_project_folder", { projectDir, format: "premiereXmeml", jobId: "nle-export-premiere-a", updatedAt: "2026-09-15T10:00:00Z" });
    expect(premiere.exportPath).toBe(`${projectDir}/exports/project-sample-premiere.xml`);
    const bundle = await request<NleXmlExportCommandResult>("export_palmier_project_package_to_split_project_folder", {
      projectDir,
      jobId: "export-palmierProject-a",
      outputPath: "exports/project-sample-palmierProject-export-palmierProject-a.palmier",
      updatedAt: "2026-09-15T10:00:01Z",
    });
    expect(bundle.project.exportArtifacts?.map((artifact) => [artifact.kind, artifact.path])).toEqual([
      ["nle_xml", "exports/project-sample-premiere.xml"],
      ["project_bundle", "exports/project-sample-palmierProject-export-palmierProject-a.palmier"],
    ]);
    const tasks = records(bundle.project).filter((task) => task.kind === "export");
    expect(tasks.map((task) => [task.label, task.status])).toEqual([
      ["Palmier Project package", "completed"],
      ["Premiere XML export", "completed"],
    ]);

    await request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: premiere.exportPath });
    await request("reveal_export_artifact_in_split_project_folder", { projectDir, artifactPath: bundle.exportPath });
    expect(window.__EDITOR_FIXTURE_REVEALS__).toHaveLength(2);
  });
});
