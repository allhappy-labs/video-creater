import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyProjectActionsLocally } from "@/lib/agent/project-merge";
import { defaultAppPreferences } from "@/lib/app-settings";
import { exportPlan, type ExportStartPlan } from "@/lib/export/export-plan";
import { draftExportDimensions, fallbackExportProfileAvailability } from "@/lib/export/profiles";
import type { MediaAsset, ProjectAction, ProjectJobSummary, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore, type EditorStore } from "../store/editor-store";
import { createExportService, loadExportProfiles, startExportRuntime } from "./export-service";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

const projectDir = "/projects/edison";
const profiles = fallbackExportProfileAvailability.map((profile) => ({ ...profile, available: true, qualityAvailability: { draft: true, final: true } }));

function job(id: string, kind: string, status: ProjectJobSummary["status"]): ProjectJobSummary {
  return { id, kind, status, updatedAt: "2026-09-15T10:00:00.000Z" };
}

function withJob(project: VideoProject, next: ProjectJobSummary): VideoProject {
  return { ...project, jobs: [...project.jobs.filter((candidate) => candidate.id !== next.id), next] };
}

function setup(handlers: Record<string, Handler> = {}, backendPreference: "inProcess" | "temporal" = "inProcess") {
  const backend = { project: { ...fixtureProject(), schemaVersion: 2 } as VideoProject };
  const all: Record<string, Handler> = {
    apply_project_actions_to_split_project_folder: (input) => {
      backend.project = applyProjectActionsLocally(backend.project, input.actions as ProjectAction[]);
      return { project: backend.project };
    },
    load_split_project_from_folder: () => backend.project,
    reveal_export_artifact_in_split_project_folder: () => undefined,
    ...handlers,
  };
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    // Handlers added after setup (they need the backend) are looked up at call time.
    const handler = handlers[command] ?? all[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
  const store = createEditorStore({ projectDir, project: backend.project });
  const service = createExportService(store, { preferences: () => ({ ...defaultAppPreferences, generationExecutionBackend: backendPreference }) });
  const plan: ExportStartPlan = exportPlan({
    choices: { name: "Edison intro", format: "mp4", resolution: "1080p", quality: "high", codec: "h264", fps: null, directory: null },
    profiles,
    project: backend.project,
    projectDir,
    jobId: "popover",
  });
  return { backend, store, service, plan };
}

/** Selects the newest toast's "Show in folder" and resolves the reveal request it sent. */
async function showInFolder(store: EditorStore) {
  const toast = store.getState().toasts[store.getState().toasts.length - 1];
  expect(toast?.action?.label).toBe("Show in folder");
  toast?.action?.onSelect();
  await vi.waitFor(() => expect(calls("reveal_export_artifact_in_split_project_folder")).toHaveLength(1));
  return calls("reveal_export_artifact_in_split_project_folder")[0]?.[1];
}

function calls(command: string) {
  return vi.mocked(backendRequest).mock.calls.filter(([name]) => name === command);
}

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  let reject: (error: unknown) => void = () => undefined;
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

describe("export service", () => {
  let cleanup: (() => void) | null = null;

  beforeEach(() => {
    installRuntimeMode("desktop");
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
  });

  afterEach(() => {
    cleanup?.();
    cleanup = null;
  });

  function track(store: EditorStore) {
    const stop = startExportRuntime(store);
    cleanup = () => {
      stop();
      store.getState().stopPolling();
    };
  }

  it("renders in process by preference: records the task, awaits the render, then toasts", async () => {
    const render = deferred<unknown>();
    const { backend, store, service, plan } = setup({ render_media_to_split_project_folder: () => render.promise });

    const exported = service.exportVideo(plan);
    await vi.waitFor(() => expect(calls("render_media_to_split_project_folder")).toHaveLength(1));
    const input = calls("render_media_to_split_project_folder")[0]?.[1] as Record<string, unknown>;
    expect(input).toMatchObject({ projectDir, projectId: backend.project.id, profile: "mp4H264", quality: "final", width: 1920, height: 1080, output: { fileName: "Edison intro", directory: null } });
    expect(input).not.toHaveProperty("fps");
    expect(input).not.toHaveProperty("encodeTier");
    expect(input.jobId).toMatch(/^export-mp4H264-/);
    expect(input.attemptId).toMatch(/^render-attempt\//);
    const jobId = input.jobId as string;
    expect(store.getState().tasks.find((task) => task.id === jobId)).toMatchObject({ kind: "export", status: "running" });
    expect(store.getState().exportPlans.get(jobId)).toMatchObject({ jobId, name: "Edison intro" });

    const completed = withJob(backend.project, job(jobId, "render_draft", "completed"));
    const report = { id: jobId, outputPath: `renders/${jobId}/output.mp4`, logPath: `renders/${jobId}/render.log` } as VideoProject["renderReports"][number];
    render.resolve({ project: { ...completed, renderReports: [report] }, outputPath: `${projectDir}/renders/${jobId}/output.mp4` });
    await expect(exported).resolves.toBe(true);
    expect(store.getState().activeRender).toBeNull();
    expect(store.getState().tasks.find((task) => task.id === jobId)).toMatchObject({ kind: "export", status: "completed" });
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Exported Edison intro"]);
    expect(calls("load_render_pipeline_report_from_split_project_folder")).toHaveLength(0);
    // An older backend records no export artifact: Show reveals the render report's recorded output.
    await expect(showInFolder(store)).resolves.toEqual({ projectDir, artifactPath: `renders/${jobId}/output.mp4` });
  });

  it("toasts and reveals the saved export file an in-process export records", async () => {
    const { backend, store, service } = setup({
      render_media_to_split_project_folder: (input) => {
        const jobId = input.jobId as string;
        const artifact = { schemaVersion: 1, id: jobId, kind: "mp4" as const, format: "mp4H264", path: "/home/me/Movies/Edison intro (2).mp4", mimeType: "video/mp4", jobId, createdAt: "2026-09-17T00:00:00.000Z" };
        backend.project = { ...withJob(backend.project, job(jobId, "render_draft", "completed")), exportArtifacts: [artifact] };
        return { project: backend.project, outputPath: `renders/${jobId}/output.mp4`, exportArtifact: artifact };
      },
    });
    const master = exportPlan({
      choices: { name: "Edison intro", format: "mp4", resolution: "1080p", quality: "master", codec: "h264", fps: 25, directory: "/home/me/Movies" },
      profiles,
      project: backend.project,
      projectDir,
      jobId: "popover",
    });

    await expect(service.exportVideo(master)).resolves.toBe(true);
    expect(calls("render_media_to_split_project_folder")[0]?.[1]).toMatchObject({ fps: 25, encodeTier: "master", output: { fileName: "Edison intro", directory: "/home/me/Movies" } });
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Exported Edison intro (2).mp4"]);
    await expect(showInFolder(store)).resolves.toEqual({ projectDir, artifactPath: "/home/me/Movies/Edison intro (2).mp4" });
  });

  it("loads the persisted failure and render report when an in-process render fails", async () => {
    let failedJobId = "";
    const { backend, store, service, plan } = setup({
      render_media_to_split_project_folder: (input) => {
        failedJobId = input.jobId as string;
        backend.project = withJob(backend.project, job(failedJobId, "render_draft", "failed"));
        throw new Error("render_media_to_split_project_folder failed");
      },
      load_render_pipeline_report_from_split_project_folder: () => ({ errors: [{ code: "font", path: "text", message: "The font Canela is missing.", fix: "Install it." }] }),
    });

    await expect(service.exportVideo(plan)).resolves.toBe(false);
    expect(calls("load_render_pipeline_report_from_split_project_folder")[0]?.[1]).toEqual({ projectDir, jobId: failedJobId });
    expect(store.getState().lastError).toBe("The font Canela is missing.");
    expect(store.getState().activeRender).toBeNull();
    expect(store.getState().tasks.find((task) => task.id === failedJobId)).toMatchObject({ kind: "export", status: "failed", retry: true });
    expect(store.getState().toasts).toEqual([]);
  });

  it("retries a failed export from its stored plan once the runner is registered", async () => {
    let attempts = 0;
    const { backend, store, service, plan } = setup({
      render_media_to_split_project_folder: (input) => {
        attempts += 1;
        const id = input.jobId as string;
        backend.project = withJob(backend.project, job(id, "render_draft", attempts === 1 ? "failed" : "completed"));
        if (attempts === 1) throw new Error("Encoder stopped.");
        return { project: backend.project, outputPath: "renders/x/output.mp4" };
      },
    });
    track(store);
    await service.exportVideo(plan);
    const failed = store.getState().tasks.find((task) => task.status === "failed");
    if (!failed) throw new Error("no failed export task");
    expect(store.getState().lastError).toBe("Encoder stopped.");

    await expect(store.getState().retryTask(failed.id)).resolves.toBe(true);
    expect(attempts).toBe(2);
    expect(store.getState().exportPopover).toBeNull();
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Exported Edison intro"]);
  });

  it("records a Draft export's chosen resolution and frame rate for Retry on both backends", async () => {
    const temporalHandlers = {
      build_temporal_export_media_start_request: (input: Record<string, unknown>) => ({ workflowId: `wf/${String(input.jobId)}`, workflowType: "ExportMediaWorkflow", taskQueue: "q", input, searchAttributes: {}, activityTypes: ["render"], idReusePolicy: "rejectDuplicate" }),
      start_temporal_workflow: (input: Record<string, unknown>) => ({ status: "started", runId: "run-6", workflowId: (input.job as ProjectJobSummary).id, workflowType: "ExportMediaWorkflow", taskQueue: "q", message: "Started" }),
      build_temporal_start_result_action: (input: Record<string, unknown>) => ({ type: "updateJobStatus", jobId: (input.job as ProjectJobSummary).id, status: "running", updatedAt: "2026-09-15T10:00:01.000Z", runId: "run-6" }),
    };
    const recorded = { profile: "mp4H264", quality: "draft", width: 1920, height: 1080, output: { fileName: "Edison intro", directory: null } };

    const temporal = setup(temporalHandlers, "temporal");
    const temporalDraft = exportPlan({ choices: { ...temporal.plan.choices, quality: "draft" }, profiles, project: temporal.backend.project, projectDir, jobId: "popover" });
    await expect(temporal.service.exportVideo(temporalDraft)).resolves.toBe(true);
    const started = calls("start_temporal_workflow")[0]?.[1] as { job: ProjectJobSummary };
    expect(calls("build_temporal_export_media_start_request")[0]?.[1]).toMatchObject({ quality: "draft", width: 1280, height: 720 });
    expect(started.job.exportSettings).toEqual(recorded);

    vi.mocked(backendRequest).mockReset();
    const render = deferred<unknown>();
    const inProcess = setup({ render_media_to_split_project_folder: () => render.promise });
    const inProcessDraft = exportPlan({ choices: { ...inProcess.plan.choices, quality: "draft" }, profiles, project: inProcess.backend.project, projectDir, jobId: "popover" });
    void inProcess.service.exportVideo(inProcessDraft);
    await vi.waitFor(() => expect(calls("render_media_to_split_project_folder")).toHaveLength(1));
    expect(calls("render_media_to_split_project_folder")[0]?.[1]).toMatchObject({ quality: "draft", width: 1280, height: 720, exportSettings: recorded });
    render.reject(new Error("stopped"));
  });

  it("refuses a blocked plan without calling the backend", async () => {
    const { service, store, plan } = setup();
    await expect(service.exportVideo({ ...plan, blockedReason: "MP4 / H.264 is not approved in this build." })).resolves.toBe(false);
    expect(store.getState().lastError).toBe("MP4 / H.264 is not approved in this build.");
    expect(backendRequest).not.toHaveBeenCalled();
  });

  it("starts a Temporal export workflow by preference and toasts once polling sees its export file recorded", async () => {
    const { backend, store, service, plan } = setup(
      {
        build_temporal_export_media_start_request: (input) => ({ workflowId: `wf/${String(input.jobId)}`, workflowType: "ExportMediaWorkflow", taskQueue: "q", input, searchAttributes: {}, activityTypes: ["render"], idReusePolicy: "rejectDuplicate" }),
        start_temporal_workflow: (input) => ({ status: "started", runId: "run-7", workflowId: (input.job as ProjectJobSummary).id, workflowType: "ExportMediaWorkflow", taskQueue: "q", message: "Started" }),
        build_temporal_start_result_action: (input) => ({ type: "updateJobStatus", jobId: (input.job as ProjectJobSummary).id, status: "running", updatedAt: "2026-09-15T10:00:01.000Z", runId: "run-7" }),
      },
      "temporal",
    );
    track(store);

    await expect(service.exportVideo(plan)).resolves.toBe(true);
    expect(calls("render_media_to_split_project_folder")).toHaveLength(0);
    const startInput = calls("build_temporal_export_media_start_request")[0]?.[1] as Record<string, unknown>;
    expect(startInput).toMatchObject({ projectDir, profile: "mp4H264", quality: "final", width: 1920, height: 1080, output: { fileName: "Edison intro", directory: null } });
    expect(startInput.outputPath).toBe(`exports/${backend.project.id}-mp4H264-${String(startInput.jobId)}.mp4`);
    const started = calls("start_temporal_workflow")[0]?.[1] as { job: ProjectJobSummary };
    expect(started.job).toMatchObject({ id: startInput.jobId, kind: "export_media", status: "queued", startRequest: { workflowType: "ExportMediaWorkflow" } });
    expect(started.job.exportSettings).toEqual({ profile: "mp4H264", quality: "final", width: 1920, height: 1080, output: { fileName: "Edison intro", directory: null } });
    expect(store.getState().project.jobs.find((candidate) => candidate.id === startInput.jobId)).toMatchObject({ status: "running" });
    expect(store.getState().toasts).toEqual([]);

    const running = backend.project.jobs.find((candidate) => candidate.id === started.job.id);
    if (!running) throw new Error("the export job was not recorded");
    // An older workflow's render step completes the job before WriteExportArtifact saves the file.
    backend.project = withJob(backend.project, { ...running, status: "completed", updatedAt: "2999-01-01T00:00:00.000Z" });
    await store.getState().mergeLoadedProject(backend.project);
    expect(store.getState().toasts).toEqual([]);

    const artifactPath = "exports/Edison intro.mp4";
    const artifact = { schemaVersion: 1, id: started.job.id, kind: "mp4" as const, format: "mp4H264", path: artifactPath, mimeType: "video/mp4", jobId: started.job.id, createdAt: "2999-01-01T00:00:01.000Z" };
    backend.project = { ...backend.project, exportArtifacts: [artifact] };
    await store.getState().mergeLoadedProject(backend.project);
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Exported Edison intro.mp4"]);
    await expect(showInFolder(store)).resolves.toEqual({ projectDir, artifactPath });
  });

  it("toasts a Temporal export's recorded artifact once polling merges it", async () => {
    const { backend, store, service } = setup(
      {
        build_temporal_export_media_start_request: (input) => ({ workflowId: `wf/${String(input.jobId)}`, workflowType: "ExportMediaWorkflow", taskQueue: "q", input, searchAttributes: {}, activityTypes: ["render"], idReusePolicy: "rejectDuplicate" }),
        start_temporal_workflow: (input) => ({ status: "started", runId: "run-8", workflowId: (input.job as ProjectJobSummary).id, workflowType: "ExportMediaWorkflow", taskQueue: "q", message: "Started" }),
        build_temporal_start_result_action: (input) => ({ type: "updateJobStatus", jobId: (input.job as ProjectJobSummary).id, status: "running", updatedAt: "2026-09-15T10:00:01.000Z", runId: "run-8" }),
      },
      "temporal",
    );
    track(store);
    const master = exportPlan({
      choices: { name: "Edison intro", format: "webm", resolution: "1080p", quality: "master", codec: "h264", fps: 50, directory: "/home/me/Movies" },
      profiles,
      project: backend.project,
      projectDir,
      jobId: "popover",
    });

    await expect(service.exportVideo(master)).resolves.toBe(true);
    const startInput = calls("build_temporal_export_media_start_request")[0]?.[1] as Record<string, unknown>;
    expect(startInput).toMatchObject({ fps: 50, encodeTier: "master", output: { fileName: "Edison intro", directory: "/home/me/Movies" } });
    const jobId = String(startInput.jobId);
    expect(backend.project.jobs.find((candidate) => candidate.id === jobId)?.exportSettings).toEqual({
      profile: "webm",
      quality: "final",
      width: 1920,
      height: 1080,
      fps: 50,
      encodeTier: "master",
      output: { fileName: "Edison intro", directory: "/home/me/Movies" },
    });

    const running = backend.project.jobs.find((candidate) => candidate.id === jobId);
    if (!running) throw new Error("the export job was not recorded");
    const artifact = { schemaVersion: 1, id: jobId, kind: "webm" as const, format: "webm", path: "/home/me/Movies/Edison intro (3).webm", mimeType: "video/webm", jobId, createdAt: "2999-01-01T00:00:00.000Z" };
    backend.project = { ...withJob(backend.project, { ...running, status: "completed", updatedAt: "2999-01-01T00:00:00.000Z" }), exportArtifacts: [artifact] };
    await store.getState().mergeLoadedProject(backend.project);
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Exported Edison intro (3).webm"]);
    await expect(showInFolder(store)).resolves.toEqual({ projectDir, artifactPath: "/home/me/Movies/Edison intro (3).webm" });
  });

  it("shows why a Temporal export could not start", async () => {
    const { store, service, plan } = setup(
      {
        build_temporal_export_media_start_request: (input) => ({ workflowId: "wf", workflowType: "ExportMediaWorkflow", taskQueue: "q", input, searchAttributes: {}, activityTypes: [], idReusePolicy: "rejectDuplicate" }),
        start_temporal_workflow: () => ({ status: "unavailable", runId: null, workflowId: "wf", workflowType: "ExportMediaWorkflow", taskQueue: "q", message: "The workflow worker isn't running." }),
      },
      "temporal",
    );
    await expect(service.exportVideo(plan)).resolves.toBe(false);
    expect(store.getState().lastError).toBe("The workflow worker isn't running.");
    expect(store.getState().tasks.find((task) => task.kind === "export")).toMatchObject({ status: "failed", failureReason: "The workflow never started." });
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("fails the export job when starting its Temporal workflow throws", async () => {
    const { store, service, plan } = setup(
      {
        build_temporal_export_media_start_request: (input) => ({ workflowId: "wf", workflowType: "ExportMediaWorkflow", taskQueue: "q", input, searchAttributes: {}, activityTypes: [], idReusePolicy: "rejectDuplicate" }),
        start_temporal_workflow: () => {
          throw new Error("Connection refused");
        },
      },
      "temporal",
    );
    await expect(service.exportVideo(plan)).resolves.toBe(false);
    expect(store.getState().lastError).toContain("Connection refused");
    expect(store.getState().project.jobs.find((job) => job.kind === "export_media")).toMatchObject({ status: "failed", failureReason: "The workflow never started." });
  });

  it.each([
    ["premiereXmeml", "Premiere XML", /^nle-export-premiere-/],
    ["davinciFcpxml", "DaVinci XML", /^nle-export-davinci-/],
  ] as const)("exports %s through its command and records the task", async (format, label, jobIdPattern) => {
    const { backend, store, service } = setup({
      export_nle_xml_to_split_project_folder: (input) => {
        backend.project = withJob(backend.project, job(input.jobId as string, "export_nle_xml", "completed"));
        return { project: backend.project, exportPath: "exports/timeline.xml", job: job(input.jobId as string, "export_nle_xml", "completed") };
      },
    });
    await expect(service.exportNleXml(format)).resolves.toBe(true);
    const input = calls("export_nle_xml_to_split_project_folder")[0]?.[1] as Record<string, unknown>;
    expect(input).toMatchObject({ projectDir, format });
    expect(input.jobId).toMatch(jobIdPattern);
    expect(store.getState().tasks.find((task) => task.id === input.jobId)).toMatchObject({ kind: "export", status: "completed" });
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual([`Exported ${label}`]);
    await expect(showInFolder(store)).resolves.toEqual({ projectDir, artifactPath: "exports/timeline.xml" });
  });

  it("exports the project package in process and records the task", async () => {
    const { backend, store, service } = setup({
      export_palmier_project_package_to_split_project_folder: (input) => {
        backend.project = withJob(backend.project, job(input.jobId as string, "export_media", "completed"));
        return { project: backend.project, exportPath: input.outputPath, job: job(input.jobId as string, "export_media", "completed") };
      },
    });
    await expect(service.exportProjectPackage()).resolves.toBe(true);
    const input = calls("export_palmier_project_package_to_split_project_folder")[0]?.[1] as Record<string, unknown>;
    expect(input.outputPath).toBe(`exports/${backend.project.id}-palmierProject-${String(input.jobId)}.palmier`);
    expect(store.getState().tasks.find((task) => task.id === input.jobId)).toMatchObject({ kind: "export", status: "completed" });
    expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Exported project package"]);
    await expect(showInFolder(store)).resolves.toEqual({ projectDir, artifactPath: input.outputPath });
  });

  it("starts the project package as a Temporal workflow by preference", async () => {
    const { store, service } = setup(
      {
        build_temporal_export_project_bundle_start_request: (input) => ({ workflowId: "wf", workflowType: "ExportMediaWorkflow", taskQueue: "q", input, searchAttributes: {}, activityTypes: ["bundle"], idReusePolicy: "rejectDuplicate" }),
        start_temporal_workflow: () => ({ status: "started", runId: "run-9", workflowId: "wf", workflowType: "ExportMediaWorkflow", taskQueue: "q", message: "Started" }),
        build_temporal_start_result_action: (input) => ({ type: "updateJobStatus", jobId: (input.job as ProjectJobSummary).id, status: "running", updatedAt: "2026-09-15T10:00:01.000Z", runId: "run-9" }),
      },
      "temporal",
    );
    track(store);
    await expect(service.exportProjectPackage()).resolves.toBe(true);
    expect(calls("export_palmier_project_package_to_split_project_folder")).toHaveLength(0);
    const input = calls("build_temporal_export_project_bundle_start_request")[0]?.[1] as Record<string, unknown>;
    expect(store.getState().tasks.find((task) => task.id === input.jobId)).toMatchObject({ kind: "export", status: "running" });
  });

  it("blocks NLE and package exports outside a split project folder", async () => {
    const store = createEditorStore({ projectDir: "", project: fixtureProject() });
    const service = createExportService(store);
    await expect(service.exportNleXml("premiereXmeml")).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Premiere XML export requires a saved split project folder.");
    await expect(service.exportProjectPackage()).resolves.toBe(false);
    expect(backendRequest).not.toHaveBeenCalled();
  });

  describe("save range as media", () => {
    const range = { startSeconds: 1, endSeconds: 3 };

    function savedMedia(jobId: string): MediaAsset {
      return { id: `media-${jobId}`, name: "Range 1", relativePath: `media/${jobId}.webm`, kind: "video", durationSeconds: 2, width: 1280, height: 720, fps: 30 };
    }

    /** The backend import: copies the render into media, named from `names`, and saves the folder under a new revision. */
    function importHandler(backend: { project: VideoProject }): Handler {
      return (input) => {
        const [sourcePath = ""] = input.sourcePaths as string[];
        const jobId = sourcePath.split("/").slice(-2, -1)[0] ?? "";
        const names = (input.names ?? {}) as Record<string, string>;
        const media = { ...savedMedia(jobId), name: names[sourcePath] ?? "output" };
        backend.project = { ...backend.project, contentRevision: (backend.project.contentRevision ?? 0) + 1, media: [...backend.project.media, media] };
        return { project: backend.project, imported: [media], skipped: [] };
      };
    }

    function renderHandler(backend: { project: VideoProject }, outcome: (jobId: string) => "completed" | "failed" = () => "completed"): Handler {
      return (input) => {
        const jobId = input.jobId as string;
        const status = outcome(jobId);
        backend.project = withJob(backend.project, job(jobId, "render_draft", status));
        if (status === "failed") throw new Error("render_media_to_split_project_folder failed");
        return { project: backend.project, outputPath: `renders/${jobId}/output.webm` };
      };
    }

    it("renders the range in process even with the Temporal preference, then imports, reveals and toasts", async () => {
      const render = deferred<unknown>();
      const handlers: Record<string, Handler> = { render_media_to_split_project_folder: () => render.promise };
      const { backend, store, service } = setup(handlers, "temporal");
      handlers.import_media_to_project = importHandler(backend);
      store.getState().setActiveTab("text");

      const saved = service.saveRangeAsMedia(range);
      await vi.waitFor(() => expect(calls("render_media_to_split_project_folder")).toHaveLength(1));
      const input = calls("render_media_to_split_project_folder")[0]?.[1] as Record<string, unknown>;
      const { width, height } = backend.project.renderSettings;
      expect(input).toMatchObject({ projectDir, projectId: backend.project.id, profile: "webm", quality: "draft", ...draftExportDimensions(width, height), rangeStartSeconds: 1, rangeEndSeconds: 3 });
      const jobId = input.jobId as string;
      expect(jobId).toMatch(/^save-range-/);
      expect(store.getState().tasks.find((task) => task.id === jobId)).toMatchObject({ kind: "render", label: "Timeline range render", status: "running" });
      expect(calls("import_media_to_project")).toHaveLength(0);

      backend.project = withJob(backend.project, job(jobId, "render_draft", "completed"));
      render.resolve({ project: backend.project, outputPath: `renders/${jobId}/output.webm` });
      await expect(saved).resolves.toBe(true);
      const sourcePath = `${projectDir}/renders/${jobId}/output.webm`;
      expect(calls("import_media_to_project")[0]?.[1]).toMatchObject({ projectDir, sourcePaths: [sourcePath], names: { [sourcePath]: "Edison Restoration Demo 00:01–00:03" } });
      expect(calls("start_temporal_workflow")).toHaveLength(0);
      const state = store.getState();
      expect(state.project.media.map((media) => media.id)).toContain(`media-${jobId}`);
      expect(state).toMatchObject({ activeTab: "media", revealMediaId: `media-${jobId}`, activeRender: null, lastError: null });
      expect(state.tasks.find((task) => task.id === jobId)).toMatchObject({ kind: "render", status: "completed" });
      expect(state.toasts.map((toast) => toast.title)).toEqual(["Saved Edison Restoration Demo 00:01–00:03 to Media"]);
    });

    it("leaves a failed task with the render's reason when the range can't render", async () => {
      const handlers: Record<string, Handler> = {
        load_render_pipeline_report_from_split_project_folder: () => ({ errors: [{ code: "font", path: "text", message: "The font Canela is missing.", fix: "Install it." }] }),
      };
      const { backend, store, service } = setup(handlers);
      handlers.render_media_to_split_project_folder = renderHandler(backend, () => "failed");

      await expect(service.saveRangeAsMedia(range)).resolves.toBe(false);
      const failed = store.getState().tasks.find((task) => task.id.startsWith("save-range-"));
      expect(failed).toMatchObject({ kind: "render", status: "failed", failureReason: "The range couldn't be saved to Media.", retry: true });
      expect(store.getState().lastError).toBe("The font Canela is missing.");
      expect(calls("import_media_to_project")).toHaveLength(0);
      expect(store.getState().toasts).toEqual([]);
    });

    it("fails the task when the output can't be imported, and Retry saves the range again", async () => {
      let imports = 0;
      const handlers: Record<string, Handler> = {};
      const { backend, store, service } = setup(handlers);
      handlers.render_media_to_split_project_folder = renderHandler(backend);
      handlers.import_media_to_project = (input) => {
        imports += 1;
        if (imports === 1) throw new Error("The disk is full.");
        return importHandler(backend)(input);
      };
      track(store);

      await expect(service.saveRangeAsMedia(range)).resolves.toBe(false);
      const failed = store.getState().tasks.find((task) => task.status === "failed");
      if (!failed) throw new Error("no failed save range task");
      expect(failed).toMatchObject({ kind: "render", failureReason: "The range couldn't be saved to Media.", retry: true });
      expect(store.getState().lastError).toBe("The range rendered but couldn't be added to Media: The disk is full.");

      await expect(store.getState().retryTask(failed.id)).resolves.toBe(true);
      const retried = calls("render_media_to_split_project_folder")[1]?.[1] as Record<string, unknown>;
      expect(retried).toMatchObject({ rangeStartSeconds: 1, rangeEndSeconds: 3 });
      expect(retried.jobId).not.toBe(failed.id);
      expect(store.getState().exportPopover).toBeNull();
      expect(store.getState().toasts.map((toast) => toast.title)).toEqual(["Saved Edison Restoration Demo 00:01–00:03 to Media"]);
    });

    it("needs a saved split project folder", async () => {
      const store = createEditorStore({ projectDir: "", project: fixtureProject() });
      await expect(createExportService(store).saveRangeAsMedia(range)).resolves.toBe(false);
      expect(store.getState().lastError).toBe("Save this project to a folder before saving a range as media.");
      expect(backendRequest).not.toHaveBeenCalled();
    });
  });

  it("loads the capability report, or unavailable profiles without a native exporter", async () => {
    setup({ get_export_profile_availability_report: () => profiles });
    await expect(loadExportProfiles()).resolves.toBe(profiles);
    setup();
    const fallback = await loadExportProfiles();
    expect(fallback.every((profile) => !profile.available && profile.unavailableReason === "Video export needs the desktop app's native exporter.")).toBe(true);
  });
});
