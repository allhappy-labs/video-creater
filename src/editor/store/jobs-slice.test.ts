import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyProjectActionsLocally } from "@/lib/agent/project-merge";
import { temporalCancelReason } from "@/lib/jobs/task-records";
import type { GeneratedAsset, ProjectAction, ProjectJobSummary, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureGeneratedAsset, fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "./editor-store";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

const projectDir = "/projects/demo";
const loadCommand = "load_split_project_from_folder";

function job(
  id: string,
  kind: string,
  status: ProjectJobSummary["status"],
  options: { runId?: string; input?: Record<string, unknown>; updatedAt?: string } = {},
): ProjectJobSummary {
  return {
    id,
    kind,
    status,
    updatedAt: options.updatedAt ?? "2026-09-15T10:00:00.000Z",
    ...(options.runId ? { workflow: { workflowId: `wf/${id}`, workflowType: "Workflow", taskQueue: "q", runId: options.runId, activityTypes: [] } } : {}),
    ...(options.input
      ? { startRequest: { workflowId: `wf/${id}`, workflowType: "Workflow", taskQueue: "q", input: { jobId: id, ...options.input }, searchAttributes: {}, activityTypes: [], idReusePolicy: "rejectDuplicate" } }
      : {}),
  };
}

function asset(id: string, status: GeneratedAsset["status"], overrides: Partial<GeneratedAsset> = {}): GeneratedAsset {
  return { ...fixtureGeneratedAsset(fixtureProject()), id, status, outputs: [], createdAt: "2026-09-15T09:00:00.000Z", ...overrides };
}

const runningTranscription = job("transcribe-1", "transcribe_media", "running", { runId: "run-1", input: { mediaId: "media-1" } });

function splitProject(overrides: Partial<VideoProject> = {}): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, contentRevision: 3, ...overrides };
}

/** A backend holding the committed project; unhandled commands are "unavailable", like a browser. */
function setup(project: VideoProject, handlers: Record<string, Handler> = {}, dir = projectDir) {
  const backend = { project };
  const all: Record<string, Handler> = {
    apply_project_actions_to_split_project_folder: (input) => {
      backend.project = applyProjectActionsLocally(backend.project, input.actions as ProjectAction[]);
      return { project: backend.project };
    },
    [loadCommand]: () => backend.project,
    ...handlers,
  };
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = all[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
  const store = createEditorStore({ projectDir: dir, project });
  const batches: ProjectAction[][] = [];
  const apply = store.getState().applyActions;
  store.setState({
    applyActions: (actions, options) => {
      batches.push([...actions]);
      return apply(actions, options);
    },
  });
  return { backend, store, batches };
}

function calls(command: string) {
  return vi.mocked(backendRequest).mock.calls.filter(([name]) => name === command);
}

function withJobStatus(project: VideoProject, jobId: string, status: ProjectJobSummary["status"]): VideoProject {
  return { ...project, jobs: project.jobs.map((entry) => (entry.id === jobId ? { ...entry, status, updatedAt: "2026-09-15T10:05:00.000Z" } : entry)) };
}

describe("jobs slice", () => {
  let stop: (() => void) | null = null;

  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
  });

  afterEach(() => {
    stop?.();
    stop = null;
    vi.useRealTimers();
  });

  describe("polling", () => {
    it("polls every second while a job runs, backs off to five seconds after a minute, and resets for a new job", async () => {
      vi.useFakeTimers();
      const { store } = setup(splitProject({ jobs: [runningTranscription] }));
      stop = () => store.getState().stopPolling();
      store.getState().startPolling();

      await vi.advanceTimersByTimeAsync(999);
      expect(calls(loadCommand)).toHaveLength(0);
      await vi.advanceTimersByTimeAsync(1);
      expect(calls(loadCommand)).toHaveLength(1);
      await vi.advanceTimersByTimeAsync(59_000);
      expect(calls(loadCommand)).toHaveLength(60);
      await vi.advanceTimersByTimeAsync(4_999);
      expect(calls(loadCommand)).toHaveLength(60);
      await vi.advanceTimersByTimeAsync(1);
      expect(calls(loadCommand)).toHaveLength(61);

      await store.getState().applyActions([{ type: "recordJob", job: job("transcribe-2", "transcribe_media", "queued", { input: { mediaId: "media-1" } }) }]);
      await vi.advanceTimersByTimeAsync(1_000);
      expect(calls(loadCommand)).toHaveLength(62);
    });

    it("shows an in-process export an earlier session left running as failed once the load reconciles it, with Retry and no undo step", async () => {
      vi.useFakeTimers();
      const stale = job("export-mp4H264-old", "render_draft", "running", { runId: "render-attempt/old", updatedAt: "2026-09-13T12:00:00.000Z" });
      const { backend, store } = setup(splitProject({ jobs: [stale] }));
      stop = () => store.getState().stopPolling();
      expect(store.getState().tasks.find((task) => task.id === "export-mp4H264-old")?.status).toBe("running");

      // The backend fails in-process jobs no runner owns any more when it loads the project.
      backend.project = withJobStatus(backend.project, "export-mp4H264-old", "failed");
      store.getState().startPolling();
      await vi.advanceTimersByTimeAsync(1_000);

      expect(store.getState().tasks.find((task) => task.id === "export-mp4H264-old")).toMatchObject({ status: "failed", retry: true });
      expect(store.getState().history.past).toHaveLength(0);
      await vi.advanceTimersByTimeAsync(10_000);
      expect(calls(loadCommand)).toHaveLength(1);
    });

    it("waits for startPolling and stops when the editor stops it", async () => {
      vi.useFakeTimers();
      const { store } = setup(splitProject({ jobs: [runningTranscription] }));

      await vi.advanceTimersByTimeAsync(3_000);
      expect(calls(loadCommand)).toHaveLength(0);
      store.getState().startPolling();
      await vi.advanceTimersByTimeAsync(1_000);
      expect(calls(loadCommand)).toHaveLength(1);
      store.getState().stopPolling();
      await vi.advanceTimersByTimeAsync(10_000);
      expect(calls(loadCommand)).toHaveLength(1);
    });

    it("stops once every job is terminal", async () => {
      vi.useFakeTimers();
      const { backend, store } = setup(splitProject({ jobs: [runningTranscription] }));
      stop = () => store.getState().stopPolling();
      store.getState().startPolling();
      await vi.advanceTimersByTimeAsync(1_000);
      expect(store.getState().tasks.find((task) => task.id === "transcribe-1")?.status).toBe("running");

      backend.project = withJobStatus(backend.project, "transcribe-1", "completed");
      await vi.advanceTimersByTimeAsync(1_000);
      expect(calls(loadCommand)).toHaveLength(2);
      expect(store.getState().tasks.find((task) => task.id === "transcribe-1")?.status).toBe("completed");

      await vi.advanceTimersByTimeAsync(10_000);
      expect(calls(loadCommand)).toHaveLength(2);
    });

    it("never polls browser sample or local projects", async () => {
      vi.useFakeTimers();
      const browser = setup(splitProject({ jobs: [runningTranscription] }), {}, "browser://bundled-sample-project");
      browser.store.getState().startPolling();
      const local = createEditorStore({ projectDir, project: { ...splitProject({ jobs: [runningTranscription] }), schemaVersion: 1 } });
      local.getState().startPolling();
      stop = () => {
        browser.store.getState().stopPolling();
        local.getState().stopPolling();
      };

      await vi.advanceTimersByTimeAsync(5_000);
      expect(calls(loadCommand)).toHaveLength(0);
    });

    it("merges polled job state without clobbering a local edit that lands first", async () => {
      vi.useFakeTimers();
      const project = splitProject({ jobs: [runningTranscription] });
      const itemId = fixtureItem(project, "video").id;
      let resolveWrite: (value: unknown) => void = () => undefined;
      const { backend, store } = setup(project, { apply_project_actions_to_split_project_folder: () => new Promise((resolve) => { resolveWrite = resolve; }) });
      stop = () => store.getState().stopPolling();
      store.getState().startPolling();

      const write = store.getState().applyActions([{ type: "updateVisualClipOpacity", itemId, opacity: 0.25 }]);
      // The poll loads the folder before the edit is committed there: the job finished, the edit is absent.
      backend.project = withJobStatus(project, "transcribe-1", "completed");
      await vi.advanceTimersByTimeAsync(1_000);
      expect(calls(loadCommand)).toHaveLength(1);

      const edited = applyProjectActionsLocally({ ...project, contentRevision: 4 }, [{ type: "updateVisualClipOpacity", itemId, opacity: 0.25 }]);
      resolveWrite({ project: edited });
      await write;
      await vi.advanceTimersByTimeAsync(0);

      const current = store.getState().project;
      expect(fixtureItem(current, "video").properties.opacity).toBe(0.25);
      expect(current.contentRevision).toBe(4);
      expect(current.jobs.find((entry) => entry.id === "transcribe-1")?.status).toBe("completed");
      expect(store.getState().history.past).toHaveLength(1);
    });

    it("drops redo history and stale selection when the folder changed elsewhere", async () => {
      vi.useFakeTimers();
      const project = splitProject({ jobs: [runningTranscription] });
      const itemId = fixtureItem(project, "video").id;
      const { backend, store } = setup(project, {
        save_split_project_to_folder: (input) => {
          backend.project = { ...(input.project as VideoProject), contentRevision: (backend.project.contentRevision ?? 0) + 1 };
          return { project: backend.project };
        },
      });
      stop = () => store.getState().stopPolling();
      await store.getState().applyActions([{ type: "updateVisualClipOpacity", itemId, opacity: 0.25 }]);
      await store.getState().undo();
      expect(store.getState().canRedo()).toBe(true);
      store.getState().selectItems([itemId]);

      backend.project = {
        ...backend.project,
        contentRevision: (backend.project.contentRevision ?? 0) + 1,
        timeline: { ...backend.project.timeline, tracks: backend.project.timeline.tracks.map((track) => ({ ...track, items: track.items.filter((item) => item.id !== itemId) })) },
      };
      store.getState().startPolling();
      await vi.advanceTimersByTimeAsync(1_000);

      expect(store.getState().project.timeline.tracks.flatMap((track) => track.items).some((item) => item.id === itemId)).toBe(false);
      expect(store.getState().canRedo()).toBe(false);
      expect(store.getState().selectedItemIds).toEqual([]);
    });
  });

  describe("cancel", () => {
    const inProcessGeneration = job("gen-live", "generate_media", "running", { runId: "in-process/gen-live", input: { mockMode: false } });
    const temporalGeneration = job("gen-temporal", "generate_media", "running", { runId: "run-temporal", input: { mockMode: false } });

    it("offers cancel only for in-process work and routes each kind to its command", async () => {
      const project = splitProject({
        jobs: [inProcessGeneration, temporalGeneration],
        generatedAssets: [asset("gen-live", "running"), asset("gen-temporal", "running")],
      });
      const { backend, store } = setup(project, {
        cancel_generate_media_in_process: () => ({ outcome: "cancelled", project: withJobStatus(backend.project, "gen-live", "cancelled") }),
        cancel_render_job_in_split_project_folder: () => ({ project: backend.project, report: {} }),
      });
      const task = (id: string) => store.getState().tasks.find((candidate) => candidate.id === id);

      expect(task("gen-live")?.cancel).toEqual({ available: true });
      expect(task("gen-temporal")?.cancel).toEqual({ available: false, reason: temporalCancelReason });
      await expect(store.getState().cancelTask("gen-temporal")).resolves.toBe(false);
      expect(calls("cancel_generate_media_in_process")).toHaveLength(0);

      await expect(store.getState().cancelTask("gen-live")).resolves.toBe(true);
      expect(calls("cancel_generate_media_in_process")[0]?.[1]).toMatchObject({ projectDir, jobId: "gen-live" });
      expect(task("gen-live")?.status).toBe("cancelled");

      store.getState().setActiveRender({ jobId: "render-draft-1", attemptId: "render-attempt/a", startedAt: "2026-09-15T10:00:00.000Z" });
      expect(task("render-draft-1")?.cancel).toEqual({ available: true });
      await expect(store.getState().cancelTask("render-draft-1")).resolves.toBe(true);
      expect(calls("cancel_render_job_in_split_project_folder")[0]?.[1]).toMatchObject({ projectDir, jobId: "render-draft-1", attemptId: "render-attempt/a" });
    });

    it("cancels the agent turn through the registered callback", async () => {
      const { store } = setup(splitProject());
      store.getState().setAgentTurn({ id: "agent-turn-1", label: "Agent edit", status: "running", phase: null, failureReason: null, updatedAt: "2026-09-15T10:00:00.000Z" });
      await expect(store.getState().cancelTask("agent-turn-1")).resolves.toBe(false);

      const cancel = vi.fn(async () => true);
      const unregister = store.getState().registerAgentCancel(cancel);
      await expect(store.getState().cancelTask("agent-turn-1")).resolves.toBe(true);
      expect(cancel).toHaveBeenCalledWith("agent-turn-1");

      unregister();
      await expect(store.getState().cancelTask("agent-turn-1")).resolves.toBe(false);
      expect(cancel).toHaveBeenCalledTimes(1);
    });
  });

  describe("retry", () => {
    it("reruns a failed generation, or retries the download when only the file is missing", async () => {
      const downloadOutput = { mediaId: "missing-output", relativePath: "generated/missing.mp4", sourceUrl: "https://cdn.example/missing.mp4", width: 1280, height: 720, durationSeconds: 5, fps: 24 };
      const project = splitProject({
        generatedAssets: [asset("gen-failed", "failed"), asset("gen-download", "completed", { outputs: [downloadOutput] })],
      });
      const { backend, store, batches } = setup(project, {
        run_generate_media_in_process: () => new Promise(() => undefined),
        retry_generated_asset_output_download_in_split_project_folder: () => ({ project: backend.project, report: {} }),
      });
      stop = () => store.getState().stopPolling();

      await expect(store.getState().retryTask("gen-download")).resolves.toBe(true);
      expect(calls("retry_generated_asset_output_download_in_split_project_folder")[0]?.[1]).toEqual({ projectDir, assetId: "gen-download", outputMediaId: "missing-output" });
      expect(batches).toHaveLength(0);

      await expect(store.getState().retryTask("gen-failed")).resolves.toBe(true);
      expect(batches[0]?.find((action) => action.type === "recordGeneratedAsset")).toMatchObject({ asset: { retryOfAssetId: "gen-failed", status: "queued" } });
    });

    it("retries and cancels an agent-recorded job-<assetId> generation through its asset", async () => {
      const live = job("job-gen-live", "generate_media", "running", { runId: "in-process/wf", input: { assetId: "gen-live", mockMode: false } });
      const project = splitProject({
        generatedAssets: [asset("gen-failed", "failed"), asset("gen-live", "running")],
        jobs: [job("job-gen-failed", "generate_media", "failed", { input: { assetId: "gen-failed", mockMode: false } }), live],
      });
      const { backend, store, batches } = setup(project, {
        run_generate_media_in_process: () => new Promise(() => undefined),
        cancel_generate_media_in_process: () => ({ outcome: "cancelled", project: withJobStatus(backend.project, "job-gen-live", "cancelled") }),
      });
      stop = () => store.getState().stopPolling();

      await expect(store.getState().retryTask("job-gen-failed")).resolves.toBe(true);
      expect(batches[0]?.find((action) => action.type === "recordGeneratedAsset")).toMatchObject({ asset: { retryOfAssetId: "gen-failed", status: "queued" } });

      await expect(store.getState().cancelTask("job-gen-live")).resolves.toBe(true);
      expect(calls("cancel_generate_media_in_process")[0]?.[1]).toMatchObject({ projectDir, jobId: "job-gen-live" });
    });

    it("re-transcribes the job's media", async () => {
      const project = splitProject({ jobs: [job("transcribe-failed", "transcribe_media", "failed", { input: { mediaId: "media-1", languageMode: "en" } })] });
      const { store, batches } = setup(project);

      await store.getState().retryTask("transcribe-failed");
      const recorded = batches[0]?.find((action) => action.type === "recordJob");
      expect(recorded).toMatchObject({ job: { kind: "transcribe_media", startRequest: { input: { mediaId: "media-1", languageMode: "en" } } } });
    });

    it("re-runs a stored export plan through the export runner, else opens the Export popover preset from the job", async () => {
      const project = splitProject({
        jobs: [
          job("export-mp4-1", "export_media", "failed", { input: { profile: "mp4H264", quality: "final" } }),
          job("render-draft-1", "render_draft", "failed"),
          job("export-mp4H265-old", "render_draft", "failed", { runId: "render-attempt/old" }),
        ],
      });
      const { store } = setup(project);

      await expect(store.getState().retryTask("export-mp4-1")).resolves.toBe(true);
      expect(store.getState().exportPopover).toEqual({ preset: { jobId: "export-mp4-1", profile: "mp4H264", quality: "final", nleFormat: null, settings: null } });
      await store.getState().retryTask("render-draft-1");
      expect(store.getState().exportPopover).toEqual({ preset: { jobId: "render-draft-1", profile: "webm", quality: "draft", nleFormat: null, settings: null } });
      // An in-process export from an earlier session has no stored plan; its job id still names the profile.
      await expect(store.getState().retryTask("export-mp4H265-old")).resolves.toBe(true);
      expect(store.getState().exportPopover).toEqual({ preset: { jobId: "export-mp4H265-old", profile: "mp4H265", quality: null, nleFormat: null, settings: null } });
      store.getState().closeExportPopover();

      const plan = { profile: "mp4H264" };
      const runner = vi.fn(async () => true);
      store.getState().registerExportRunner(runner);
      store.getState().rememberExportPlan("export-mp4-1", plan);
      await expect(store.getState().retryTask("export-mp4-1")).resolves.toBe(true);
      expect(runner).toHaveBeenCalledWith(plan, "export-mp4-1");
      expect(store.getState().exportPopover).toBeNull();
    });

    it("presets Retry from the job's recorded export settings", async () => {
      const settings = { profile: "webm" as const, quality: "final" as const, width: 3840, height: 2160, fps: 25, encodeTier: "master" as const, output: { fileName: "Edison master", directory: "/home/me/Movies" } };
      const project = splitProject({
        jobs: [
          { ...job("export-webm-1", "render_draft", "failed", { runId: "render-attempt/a" }), exportSettings: settings },
          job("export-mp4H264-2", "export_media", "failed", {
            input: { profile: "mp4H264", quality: "final", width: 1920, height: 1080, fps: 50, encodeTier: "master", destination: { fileName: "Edison intro" } },
          }),
        ],
      });
      const { store } = setup(project);

      await expect(store.getState().retryTask("export-webm-1")).resolves.toBe(true);
      expect(store.getState().exportPopover).toEqual({ preset: { jobId: "export-webm-1", profile: "webm", quality: "final", nleFormat: null, settings } });
      await expect(store.getState().retryTask("export-mp4H264-2")).resolves.toBe(true);
      expect(store.getState().exportPopover?.preset?.settings).toEqual({
        profile: "mp4H264",
        quality: "final",
        width: 1920,
        height: 1080,
        fps: 50,
        encodeTier: "master",
        output: { fileName: "Edison intro" },
      });
    });

    it("ignores tasks that are not retryable", async () => {
      const project = splitProject({ jobs: [job("analysis-1", "analyze_media", "failed"), runningTranscription] });
      const { store, batches } = setup(project);
      await expect(store.getState().retryTask("analysis-1")).resolves.toBe(false);
      await expect(store.getState().retryTask("transcribe-1")).resolves.toBe(false);
      await expect(store.getState().retryTask("missing")).resolves.toBe(false);
      expect(batches).toHaveLength(0);
    });
  });

  it("opens task details", () => {
    const store = createEditorStore({ projectDir, project: splitProject() });
    store.getState().openTaskDetails("transcribe-1");
    expect(store.getState().taskDetailsId).toBe("transcribe-1");
    store.getState().closeTaskDetails();
    expect(store.getState().taskDetailsId).toBeNull();
  });
});

describe("jobs slice monitors", () => {
  let stop: (() => void) | null = null;
  const progressCommand = "load_job_progress_from_split_project_folder";
  const reconcileCommand = "reconcile_temporal_jobs_in_split_project_folder";
  const runningExport = job("export-mp4H264-1", "export_media", "running", { runId: "run-1", input: { profile: "mp4H264", quality: "final" } });

  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.useFakeTimers();
  });

  afterEach(() => {
    stop?.();
    stop = null;
    vi.useRealTimers();
  });

  it("starts both monitors with polling and stops them with it", async () => {
    const reachable = { project: null, failedJobIds: [], serviceReachable: true, detail: null };
    const { store } = setup(splitProject({ jobs: [runningExport] }), { [progressCommand]: () => [], [reconcileCommand]: () => reachable });

    store.getState().startPolling();
    await vi.advanceTimersByTimeAsync(500);
    expect(calls(reconcileCommand)).toHaveLength(1);
    expect(calls(progressCommand)).toHaveLength(1);

    store.getState().stopPolling();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(calls(reconcileCommand)).toHaveLength(1);
    expect(calls(progressCommand)).toHaveLength(1);
  });

  it("shows progress snapshots on running tasks without touching the project or its history", async () => {
    // The folder reload never resolves, as while a render holds the project lease.
    const { store } = setup(splitProject({ jobs: [runningExport] }), {
      [loadCommand]: () => new Promise(() => undefined),
      [progressCommand]: () => [{ jobId: "export-mp4H264-1", progress: 0.42, updatedAt: "2026-09-15T10:00:01.000Z" }],
    });
    stop = () => store.getState().stopPolling();
    const project = store.getState().project;

    store.getState().startPolling();
    await vi.advanceTimersByTimeAsync(1_500);

    expect(store.getState().jobProgress.get("export-mp4H264-1")).toBe(0.42);
    expect(store.getState().tasks.find((task) => task.id === "export-mp4H264-1")?.progress).toBe(0.42);
    expect(store.getState().project).toBe(project);
    expect(store.getState().history.past).toHaveLength(0);
    expect(calls(progressCommand).length).toBeGreaterThanOrEqual(2);
  });

  it("merges a reconciliation that failed a stale job and shows its reason", async () => {
    const queued = job("export-stale", "export_nle_xml", "queued", { input: { format: "davinciFcpxml" }, updatedAt: "2026-09-15T09:00:00.000Z" });
    const { backend, store } = setup(splitProject({ jobs: [queued] }), {
      [reconcileCommand]: () => {
        backend.project = {
          ...backend.project,
          contentRevision: (backend.project.contentRevision ?? 0) + 1,
          jobs: backend.project.jobs.map((entry) => ({ ...entry, status: "failed", updatedAt: "2026-09-15T10:00:00.000Z", failureReason: "The workflow never started." })),
        };
        return { project: backend.project, failedJobIds: ["export-stale"], serviceReachable: true, detail: null };
      },
    });
    stop = () => store.getState().stopPolling();

    store.getState().startPolling();
    await vi.advanceTimersByTimeAsync(0);

    expect(store.getState().tasks.find((task) => task.id === "export-stale")).toMatchObject({ status: "failed", failureReason: "The workflow never started." });
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("marks Temporal-backed tasks while the workflow service is unreachable, and clears it once reachable", async () => {
    let reachable = false;
    const { store } = setup(splitProject({ jobs: [runningTranscription] }), {
      [reconcileCommand]: () =>
        reachable
          ? { project: null, failedJobIds: [], serviceReachable: true, detail: null }
          : { project: null, failedJobIds: [], serviceReachable: false, detail: "Workflow service unreachable" },
    });
    stop = () => store.getState().stopPolling();

    store.getState().startPolling();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.getState().workflowServiceIssue).toBe("Workflow service unreachable");
    expect(store.getState().tasks.find((task) => task.id === "transcribe-1")?.detail).toBe("Workflow service unreachable");

    reachable = true;
    await vi.advanceTimersByTimeAsync(30_000);
    expect(store.getState().workflowServiceIssue).toBeNull();
    expect(store.getState().tasks.find((task) => task.id === "transcribe-1")?.detail).toBe("Transcribing…");
  });

  it("shows the reconciliation detail for a missing namespace, and the generic text when none is given", async () => {
    let detail: string | null = 'Workflow namespace "video-creater" not found';
    const { store } = setup(splitProject({ jobs: [runningTranscription] }), {
      [reconcileCommand]: () => ({ project: null, failedJobIds: [], serviceReachable: false, detail }),
    });
    stop = () => store.getState().stopPolling();

    store.getState().startPolling();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.getState().tasks.find((task) => task.id === "transcribe-1")?.detail).toBe('Workflow namespace "video-creater" not found');

    detail = null;
    await vi.advanceTimersByTimeAsync(30_000);
    expect(store.getState().tasks.find((task) => task.id === "transcribe-1")?.detail).toBe("Workflow service unreachable");
  });
});
