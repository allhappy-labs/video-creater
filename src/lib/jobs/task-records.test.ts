import { describe, expect, it } from "vitest";
import { taskRecords, temporalCancelReason, workflowServiceUnreachableDetail, type TaskRecord, type TaskRuntime } from "@/lib/jobs/task-records";
import type {
  GeneratedAsset,
  ProjectExportArtifact,
  ProjectJobSummary,
  ProjectRenderReport,
  TemporalWorkflowStartRequest,
  VideoProject,
} from "@/lib/project";
import { fixtureGeneratedAsset, fixtureProject } from "@/test-utils/editor-fixtures";

const finishedReason = "This task is no longer running.";
const unsupportedReason = "This task can't be cancelled from the editor.";

function runtime(overrides: Partial<TaskRuntime> = {}): TaskRuntime {
  return {
    projectDir: "/projects/demo",
    executionBackend: "temporal",
    activeRender: null,
    agentTurn: null,
    exportPlanJobIds: new Set(),
    progressByJobId: new Map(),
    workflowServiceIssue: null,
    ...overrides,
  };
}

function emptyProject(): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, jobs: [], generatedAssets: [], renderReports: [], exportArtifacts: [] };
}

function startRequest(input: Record<string, unknown>): TemporalWorkflowStartRequest {
  return {
    workflowId: `video-creater/demo/${String(input.jobId ?? "job")}`,
    workflowType: "Workflow",
    taskQueue: "video-creater-workflows",
    input,
    searchAttributes: {},
    activityTypes: [],
    idReusePolicy: "rejectDuplicate",
  };
}

function job(
  id: string,
  kind: string,
  status: ProjectJobSummary["status"],
  options: { runId?: string | null; input?: Record<string, unknown>; updatedAt?: string } = {},
): ProjectJobSummary {
  const workflowId = `video-creater/demo/${id}`;
  return {
    id,
    kind,
    status,
    updatedAt: options.updatedAt ?? "2026-09-15T10:00:00Z",
    ...(options.runId === undefined
      ? {}
      : { workflow: { workflowId, workflowType: "Workflow", taskQueue: "video-creater-workflows", runId: options.runId, activityTypes: [] } }),
    ...(options.input ? { startRequest: startRequest({ jobId: id, ...options.input }) } : {}),
  };
}

function asset(id: string, status: GeneratedAsset["status"], overrides: Partial<GeneratedAsset> = {}): GeneratedAsset {
  return { ...fixtureGeneratedAsset(fixtureProject()), id, status, name: `Shot ${id}`, outputs: [], createdAt: "2026-09-15T09:00:00Z", ...overrides };
}

function report(id: string, status: ProjectRenderReport["status"]): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id,
    status,
    outputPath: `renders/${id}/output.webm`,
    durationSeconds: 4,
    streams: { video: true, audio: true },
    checks: {},
    artifacts: [],
    logPath: `renders/${id}/render.log`,
    createdAt: "2026-09-15T10:00:00Z",
  };
}

function artifact(id: string, jobId: string): ProjectExportArtifact {
  return { schemaVersion: 1, id, kind: "mp4", format: "mp4", path: `exports/${id}.mp4`, mimeType: "video/mp4", jobId, createdAt: "2026-09-15T10:00:00Z" };
}

function only(records: TaskRecord[], id: string): TaskRecord {
  const record = records.find((candidate) => candidate.id === id);
  if (!record) throw new Error(`no task record ${id}`);
  return record;
}

describe("taskRecords", () => {
  it("maps a running Temporal transcription with a disabled cancel", () => {
    const project = { ...emptyProject(), jobs: [job("transcribe-1", "transcribe_media", "running", { runId: "run-1", input: { mediaId: "media-1" } })] };

    expect(taskRecords(project, runtime())).toEqual([
      {
        id: "transcribe-1",
        kind: "transcription",
        label: "Transcribe input.mp4",
        status: "running",
        progress: null,
        detail: "Transcribing…",
        failureReason: null,
        artifactPath: null,
        logPath: null,
        cancel: { available: false, reason: temporalCancelReason },
        retry: false,
        updatedAt: "2026-09-15T10:00:00Z",
        workflow: { runId: "run-1", workflowId: "video-creater/demo/transcribe-1", backend: "temporal" },
      },
    ]);
  });

  it("maps progress to running and queued Temporal jobs to the Temporal cancel reason", () => {
    const project = {
      ...emptyProject(),
      jobs: [
        job("transcribe-1", "transcribe_media", "progress", { runId: "run-1", input: { mediaId: "media-1" } }),
        job("transcribe-2", "transcribe_media", "queued", { runId: null, input: { mediaId: "media-1" } }),
      ],
    };

    const records = taskRecords(project, runtime());

    expect(only(records, "transcribe-1").status).toBe("running");
    expect(only(records, "transcribe-2")).toMatchObject({ status: "queued", detail: "Queued", cancel: { available: false, reason: temporalCancelReason } });
  });

  it("offers retry for a failed transcription while its media is still in the project", () => {
    const project = {
      ...emptyProject(),
      jobs: [
        job("transcribe-1", "transcribe_media", "failed", { runId: "run-1", input: { mediaId: "media-1" } }),
        job("transcribe-2", "transcribe_media", "failed", { runId: "run-2", input: { mediaId: "gone" } }),
      ],
    };

    const records = taskRecords(project, runtime());

    expect(only(records, "transcribe-1")).toMatchObject({
      detail: null,
      failureReason: "The transcription stopped before it finished.",
      retry: true,
      cancel: { available: false, reason: finishedReason },
    });
    expect(only(records, "transcribe-2")).toMatchObject({ label: "Transcribe media", retry: false });
  });

  it("maps unknown job kinds to analysis without retry", () => {
    const project = { ...emptyProject(), jobs: [job("analysis-1", "analyze_speech", "failed", { runId: "run-1" })] };

    expect(only(taskRecords(project, runtime()), "analysis-1")).toMatchObject({
      kind: "analysis",
      label: "Analyze speech",
      failureReason: "The analysis stopped before it finished.",
      retry: false,
    });
  });

  it("leaves canonical preview frame captures out of the background tasks", () => {
    const project = { ...emptyProject(), jobs: [job("frame-1", "captureCanonicalPreviewFrame", "failed"), job("analysis-1", "analyze_speech", "completed")] };

    expect(taskRecords(project, runtime()).map((record) => record.id)).toEqual(["analysis-1"]);
  });

  it("lets a real in-process generation be cancelled", () => {
    const base = emptyProject();
    const project = {
      ...base,
      generatedAssets: [asset("gen-1", "running")],
      jobs: [job("gen-1", "generate_media", "running", { runId: "in-process/video-creater/demo/gen-1", input: { mockMode: false } })],
    };

    expect(only(taskRecords(project, runtime()), "gen-1")).toMatchObject({
      kind: "generation",
      label: "Shot gen-1",
      detail: "Generating…",
      cancel: { available: true },
      workflow: { backend: "inProcess", runId: "in-process/video-creater/demo/gen-1" },
    });
  });

  it("joins an agent-recorded job-<assetId> generation to its asset: one cancellable task, labelled by the asset", () => {
    const base = emptyProject();
    const project = {
      ...base,
      generatedAssets: [asset("gen-1", "running")],
      jobs: [job("job-gen-1", "generate_media", "running", { runId: "in-process/video-creater/demo/job-gen-1", input: { assetId: "gen-1", mockMode: false } })],
    };

    const records = taskRecords(project, runtime());

    expect(records.map((record) => record.id)).toEqual(["job-gen-1"]);
    expect(only(records, "job-gen-1")).toMatchObject({ kind: "generation", label: "Shot gen-1", cancel: { available: true } });
  });

  it("does not cancel mock, projectless or Temporal generations", () => {
    const base = emptyProject();
    const inProcess = { runId: "in-process/video-creater/demo/gen-1", input: { mockMode: false } };
    const project = (generationJob: ProjectJobSummary) => ({ ...base, generatedAssets: [asset("gen-1", "running")], jobs: [generationJob] });

    expect(only(taskRecords(project(job("gen-1", "generate_media", "running", { ...inProcess, input: { mockMode: true } })), runtime()), "gen-1").cancel).toEqual({
      available: false,
      reason: unsupportedReason,
    });
    expect(only(taskRecords(project(job("gen-1", "generate_media", "running", inProcess)), runtime({ projectDir: " " })), "gen-1").cancel).toEqual({
      available: false,
      reason: unsupportedReason,
    });
    expect(only(taskRecords(project(job("gen-1", "generate_media", "running", { runId: "run-1", input: { mockMode: false } })), runtime()), "gen-1").cancel).toEqual({
      available: false,
      reason: temporalCancelReason,
    });
  });

  it("uses the execution backend preference for a generation that has not started", () => {
    const base = emptyProject();
    const project = { ...base, generatedAssets: [asset("gen-1", "queued")], jobs: [job("gen-1", "generate_media", "queued", { runId: null, input: { mockMode: false } })] };

    expect(only(taskRecords(project, runtime({ executionBackend: "inProcess" })), "gen-1")).toMatchObject({
      workflow: { backend: "inProcess" },
      cancel: { available: false, reason: unsupportedReason },
    });
    expect(only(taskRecords(project, runtime()), "gen-1").cancel).toEqual({ available: false, reason: temporalCancelReason });
  });

  it("maps completed and failed generations with outputs, retry and download failures", () => {
    const base = emptyProject();
    const output = { mediaId: "media-1", relativePath: "generated/shot.mp4", width: 640, height: 360, durationSeconds: 4, fps: 24 };
    const missing = { ...output, mediaId: "missing-media", sourceUrl: "https://provider.example/shot.mp4" };
    const project = {
      ...base,
      generatedAssets: [
        asset("gen-done", "completed", { outputs: [output] }),
        asset("gen-failed", "failed", { prompt: "A hero shot" }),
        asset("gen-no-prompt", "failed", { prompt: "  " }),
        asset("gen-download", "completed", { prompt: " ", outputs: [missing] }),
      ],
      jobs: [
        job("gen-done", "generate_media", "completed", { runId: "run-1" }),
        job("gen-failed", "generate_media", "failed", { runId: "run-2" }),
        job("gen-no-prompt", "generate_media", "failed", { runId: "run-3" }),
        job("gen-download", "generate_media", "completed", { runId: "run-4" }),
      ],
    };

    const records = taskRecords(project, runtime());

    expect(only(records, "gen-done")).toMatchObject({ status: "completed", detail: "Completed", artifactPath: "generated/shot.mp4", retry: false });
    expect(only(records, "gen-failed")).toMatchObject({ failureReason: "The generation stopped before it finished.", retry: true, artifactPath: null });
    expect(only(records, "gen-no-prompt").retry).toBe(false);
    expect(only(records, "gen-download")).toMatchObject({ status: "failed", failureReason: "The generated file couldn't be downloaded.", retry: true });
  });

  it("offers Retry on in-process work an earlier session left unfinished once the backend fails it", () => {
    const project = {
      ...emptyProject(),
      generatedAssets: [asset("gen-stale", "failed", { prompt: "A hero shot" })],
      jobs: [
        job("export-mp4H264-stale", "render_draft", "failed", { runId: "render-attempt/a" }),
        job("save-range-stale", "render_draft", "failed", { runId: "render-attempt/b" }),
        job("export-palmierProject-stale", "export_media", "failed", { runId: null, input: { profile: "palmierProject" } }),
        job("gen-stale", "generate_media", "failed", { runId: "in-process/video-creater/demo/gen-stale", input: { assetId: "gen-stale" } }),
        job("frame-stale", "captureCanonicalPreviewFrame", "failed", { runId: "render-attempt/c" }),
      ],
    };

    const records = taskRecords(project, runtime({ executionBackend: "inProcess" }));

    expect(only(records, "export-mp4H264-stale")).toMatchObject({ status: "failed", retry: true, workflow: { backend: "inProcess" } });
    expect(only(records, "save-range-stale")).toMatchObject({ status: "failed", failureReason: "The range couldn't be saved to Media.", retry: true });
    expect(only(records, "export-palmierProject-stale")).toMatchObject({ kind: "export", status: "failed", retry: true });
    expect(only(records, "gen-stale")).toMatchObject({ kind: "generation", status: "failed", retry: true, workflow: { backend: "inProcess" } });
    expect(records.some((record) => record.id === "frame-stale")).toBe(false);
  });

  it("takes a terminal generated asset status over a job that is still running", () => {
    const base = emptyProject();
    const project = { ...base, generatedAssets: [asset("gen-1", "cancelled")], jobs: [job("gen-1", "generate_media", "running", { runId: "run-1" })] };

    expect(only(taskRecords(project, runtime()), "gen-1")).toMatchObject({ status: "cancelled", detail: "Cancelled" });
  });

  it("includes generated assets without jobs", () => {
    const base = emptyProject();
    const project = { ...base, generatedAssets: [asset("orphan", "failed", { prompt: "Retry me" })] };

    expect(taskRecords(project, runtime())).toEqual([
      {
        id: "orphan",
        kind: "generation",
        label: "Shot orphan",
        status: "failed",
        progress: null,
        detail: null,
        failureReason: "The generation stopped before it finished.",
        artifactPath: null,
        logPath: null,
        cancel: { available: false, reason: finishedReason },
        retry: true,
        updatedAt: "2026-09-15T09:00:00Z",
        workflow: null,
      },
    ]);
  });

  it("maps in-process renders: the active render is cancellable, failed ones retry with a log", () => {
    const project = {
      ...emptyProject(),
      renderReports: [report("render-old", "failed")],
      jobs: [
        job("render-active", "render_draft", "running", { runId: "render-attempt/a" }),
        job("render-old", "render_draft", "failed", { runId: "render-attempt/b" }),
        job("save-range-1", "render_draft", "running", { runId: "render-attempt/c" }),
      ],
    };

    const records = taskRecords(project, runtime({ activeRender: { jobId: "render-active", attemptId: "render-attempt/a", startedAt: "2026-09-15T10:00:00Z" } }));

    expect(only(records, "render-active")).toMatchObject({ kind: "render", label: "Timeline render", detail: "Rendering…", cancel: { available: true } });
    expect(only(records, "render-old")).toMatchObject({
      failureReason: "The render stopped before it finished.",
      logPath: "renders/render-old/render.log",
      retry: true,
      workflow: { backend: "inProcess" },
    });
    expect(only(records, "save-range-1")).toMatchObject({ label: "Timeline range render", cancel: { available: false, reason: unsupportedReason } });
  });

  it("keeps a saved range a render task with its stored retry plan, saying what failed", () => {
    const project = { ...emptyProject(), jobs: [job("save-range-2", "render_draft", "failed", { runId: "render-attempt/d" })] };
    const plans = runtime({ exportPlanJobIds: new Set(["save-range-2", "save-range-3"]) });

    expect(only(taskRecords(project, plans), "save-range-2")).toMatchObject({
      kind: "render",
      label: "Timeline range render",
      failureReason: "The range couldn't be saved to Media.",
      retry: true,
    });
    const active = taskRecords(emptyProject(), { ...plans, activeRender: { jobId: "save-range-3", attemptId: "render-attempt/e", startedAt: "2026-09-15T11:00:00Z" } });
    expect(only(active, "save-range-3")).toMatchObject({ kind: "render", label: "Timeline range render", detail: "Rendering…" });
  });

  it("does not retry a failed Temporal render with a start request", () => {
    const project = { ...emptyProject(), jobs: [job("render-1", "render_draft", "failed", { runId: "run-1", input: { quality: "draft" } })] };

    expect(only(taskRecords(project, runtime()), "render-1").retry).toBe(false);
  });

  it("adds the active in-process render before the backend records its job", () => {
    const records = taskRecords(emptyProject(), runtime({ activeRender: { jobId: "render-new", attemptId: "render-attempt/x", startedAt: "2026-09-15T11:00:00Z" } }));

    expect(records).toEqual([
      expect.objectContaining({
        id: "render-new",
        kind: "render",
        status: "running",
        cancel: { available: true },
        updatedAt: "2026-09-15T11:00:00Z",
        workflow: { runId: "render-attempt/x", backend: "inProcess" },
      }),
    ]);
  });

  it("maps exports: stored-plan renders, Temporal media, NLE XML and packages", () => {
    const project = {
      ...emptyProject(),
      exportArtifacts: [artifact("artifact-1", "export-mp4H264-1")],
      jobs: [
        job("render-final-1", "render_draft", "running", { runId: "render-attempt/a" }),
        job("export-mp4H264-1", "export_media", "completed", { runId: "run-1", input: { profile: "mp4H264", quality: "final" } }),
        job("export-palmierProject-1", "export_media", "failed", { runId: "run-2", input: { profile: "palmierProject" } }),
        job("nle-export-premiere-1", "export_nle_xml", "queued", { runId: null, input: { format: "premiereXmeml" } }),
        job("legacy-export", "exportMedia", "failed", { runId: "run-3" }),
      ],
    };

    const records = taskRecords(project, runtime({ exportPlanJobIds: new Set(["render-final-1"]) }));

    expect(only(records, "render-final-1")).toMatchObject({ kind: "export", label: "Video export", detail: "Exporting…" });
    expect(only(records, "export-mp4H264-1")).toMatchObject({ kind: "export", label: "H.264 Final export", artifactPath: "exports/artifact-1.mp4" });
    expect(only(records, "export-palmierProject-1")).toMatchObject({
      label: "Palmier Project package",
      failureReason: "The export stopped before it finished.",
      retry: true,
    });
    expect(only(records, "nle-export-premiere-1")).toMatchObject({ label: "Premiere XML export", cancel: { available: false, reason: temporalCancelReason } });
    expect(only(records, "legacy-export")).toMatchObject({ kind: "export", label: "Video export", retry: true });
  });

  it("labels an in-process export from its recorded settings and shows its saved file", () => {
    const settings = { profile: "mp4H264" as const, quality: "final" as const, width: 1920, height: 1080, encodeTier: "master" as const, output: { fileName: "Edison intro", directory: "/home/me/Movies" } };
    const project = {
      ...emptyProject(),
      renderReports: [{ ...report("export-mp4H264-2", "completed"), outputPath: "renders/export-mp4H264-2/output.mp4" }],
      exportArtifacts: [{ ...artifact("export-mp4H264-2", "export-mp4H264-2"), path: "/home/me/Movies/Edison intro (2).mp4" }],
      jobs: [{ ...job("export-mp4H264-2", "render_draft", "completed", { runId: "render-attempt/b" }), exportSettings: settings }],
    };

    const record = only(taskRecords(project, runtime()), "export-mp4H264-2");

    expect(record).toMatchObject({ kind: "export", label: "H.264 Master export", artifactPath: "/home/me/Movies/Edison intro (2).mp4" });
  });

  it("keeps an in-process export an earlier session left failed an export after its stored plan is gone", () => {
    const project = {
      ...emptyProject(),
      jobs: [job("export-mp4H264-old", "render_draft", "failed", { runId: "render-attempt/old" }), job("render-draft-old", "render_draft", "failed", { runId: "render-attempt/older" })],
    };

    const records = taskRecords(project, runtime());

    expect(only(records, "export-mp4H264-old")).toMatchObject({ kind: "export", label: "Video export", failureReason: "The export stopped before it finished.", retry: true });
    expect(only(records, "render-draft-old")).toMatchObject({ kind: "render", label: "Timeline render" });
  });

  it("maps codex edit jobs and the in-memory agent turn", () => {
    const project = { ...emptyProject(), jobs: [job("codex-edit-1", "codex_edit", "running", { runId: "run-1", input: {} })] };
    const turn = { id: "turn-1", label: "Tighten the pacing", status: "running" as const, phase: "Reviewing the timeline", failureReason: null, updatedAt: "2026-09-15T10:00:00Z" };

    const records = taskRecords(project, runtime({ agentTurn: turn }));

    expect(only(records, "codex-edit-1")).toMatchObject({ kind: "agent", label: "Agent edit", cancel: { available: false, reason: temporalCancelReason } });
    expect(only(records, "turn-1")).toMatchObject({
      kind: "agent",
      label: "Tighten the pacing",
      detail: "Reviewing the timeline",
      cancel: { available: true },
      retry: false,
      workflow: null,
    });
    expect(
      only(taskRecords(emptyProject(), runtime({ agentTurn: { ...turn, status: "failed", failureReason: null } })), "turn-1"),
    ).toMatchObject({ failureReason: "The agent couldn't finish this edit.", cancel: { available: false, reason: finishedReason } });
  });

  it("orders running, queued, failed, then completed, newest first within each group", () => {
    const project = {
      ...emptyProject(),
      jobs: [
        job("completed-old", "render_draft", "completed", { updatedAt: "2026-09-15T10:00:01Z" }),
        job("failed", "render_draft", "failed", { updatedAt: "2026-09-15T10:00:09Z" }),
        job("queued", "transcribe_media", "queued", { updatedAt: "2026-09-15T10:00:02Z", input: {} }),
        job("running-old", "transcribe_media", "running", { updatedAt: "2026-09-15T10:00:03Z", runId: "run-1" }),
        job("cancelled-new", "render_draft", "cancelled", { updatedAt: "2026-09-15T10:00:08Z" }),
        job("blocked", "transcribe_media", "blocked", { updatedAt: "2026-09-15T10:00:05Z", input: {} }),
        job("running-new", "transcribe_media", "progress", { updatedAt: "2026-09-15T10:00:04Z", runId: "run-2" }),
      ],
    };

    expect(taskRecords(project, runtime()).map((record) => record.id)).toEqual([
      "running-new",
      "running-old",
      "blocked",
      "queued",
      "failed",
      "cancelled-new",
      "completed-old",
    ]);
  });
});

describe("taskRecords progress, failure reasons and workflow service reachability", () => {
  it("shows a running job's progress snapshot and ignores snapshots of other statuses", () => {
    const project = {
      ...emptyProject(),
      jobs: [
        job("export-running", "export_media", "running", { runId: "run-1", input: { profile: "mp4H264" } }),
        job("export-progress", "export_media", "progress", { runId: "run-2", input: { profile: "mp4H264" } }),
        job("export-queued", "export_media", "queued", { runId: null, input: { profile: "mp4H264" } }),
        job("export-completed", "export_media", "completed", { runId: "run-3", input: { profile: "mp4H264" } }),
        job("export-failed", "export_media", "failed", { runId: "run-4", input: { profile: "mp4H264" } }),
      ],
    };
    const progressByJobId = new Map([
      ["export-running", 0.42],
      ["export-progress", 0.5],
      ["export-queued", 0.1],
      ["export-completed", 1],
      ["export-failed", 0.3],
    ]);

    const records = taskRecords(project, runtime({ progressByJobId }));

    expect(only(records, "export-running").progress).toBe(0.42);
    expect(only(records, "export-progress").progress).toBe(0.5);
    expect(only(records, "export-queued").progress).toBeNull();
    expect(only(records, "export-completed").progress).toBeNull();
    expect(only(records, "export-failed").progress).toBeNull();
  });

  it("takes the active in-process render's progress from the snapshots", () => {
    const records = taskRecords(
      emptyProject(),
      runtime({
        activeRender: { jobId: "export-video-mp4H264-1", attemptId: "render-attempt/1", startedAt: "2026-09-15T10:00:00Z" },
        progressByJobId: new Map([["export-video-mp4H264-1", 0.25]]),
      }),
    );

    expect(records).toHaveLength(1);
    expect(records[0]?.progress).toBe(0.25);
  });

  it("shows a recorded failure reason and keeps the kind default without one", () => {
    const stale = { ...job("export-stale", "export_media", "failed", { runId: null, input: { profile: "mp4H264" } }), failureReason: "The workflow never started." };
    const plain = job("export-plain", "export_media", "failed", { runId: "run-1", input: { profile: "mp4H264" } });

    const records = taskRecords({ ...emptyProject(), jobs: [stale, plain] }, runtime());

    expect(only(records, "export-stale").failureReason).toBe("The workflow never started.");
    expect(only(records, "export-plain").failureReason).toBe("The export stopped before it finished.");
  });

  it("marks queued and running Temporal-backed tasks when the workflow service is unreachable", () => {
    const project = {
      ...emptyProject(),
      jobs: [
        job("transcribe-running", "transcribe_media", "running", { runId: "run-1", input: { mediaId: "media-1" } }),
        job("transcribe-queued", "transcribe_media", "queued", { runId: null, input: { mediaId: "media-1" } }),
        job("transcribe-done", "transcribe_media", "completed", { runId: "run-2", input: { mediaId: "media-1" } }),
        job("generate-local", "generate_media", "running", { runId: "in-process/generate-local", input: { assetId: "generate-local" } }),
      ],
    };

    const unreachable = taskRecords(project, runtime({ workflowServiceIssue: workflowServiceUnreachableDetail }));
    const reachable = taskRecords(project, runtime());

    expect(only(unreachable, "transcribe-running").detail).toBe("Workflow service unreachable");
    expect(only(unreachable, "transcribe-queued").detail).toBe("Workflow service unreachable");
    expect(only(unreachable, "transcribe-done").detail).toBe("Completed");
    expect(only(unreachable, "generate-local").detail).toBe(only(reachable, "generate-local").detail);
    expect(only(reachable, "transcribe-running").detail).toBe("Transcribing…");
  });

  it("treats a mock Temporal run as local work, as reconciliation does", () => {
    const project = { ...emptyProject(), jobs: [job("transcribe-mock", "transcribe_media", "running", { runId: "mock-run-transcribe-mock", input: { mediaId: "media-1" } })] };

    const record = only(taskRecords(project, runtime({ workflowServiceIssue: workflowServiceUnreachableDetail })), "transcribe-mock");

    expect(record.workflow).toMatchObject({ backend: "inProcess", runId: "mock-run-transcribe-mock" });
    expect(record.detail).toBe("Transcribing…");
  });

  it("shows why the workflow service can't be used on active Temporal-backed tasks", () => {
    const project = { ...emptyProject(), jobs: [job("transcribe-running", "transcribe_media", "running", { runId: "run-1", input: { mediaId: "media-1" } })] };

    const records = taskRecords(project, runtime({ workflowServiceIssue: 'Workflow namespace "video-creater" not found' }));

    expect(only(records, "transcribe-running").detail).toBe('Workflow namespace "video-creater" not found');
  });
});
