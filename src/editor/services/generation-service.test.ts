import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyProjectActionsLocally } from "@/lib/agent/project-merge";
import { defaultAppPreferences, type AppSettingsPreferences } from "@/lib/app-settings";
import { generatedTimelinePlaceholderAction } from "@/lib/generation/timeline-placement";
import type { MediaGenerationRequest } from "@/lib/generation/types";
import { buildFallbackGenerateMediaStartRequest } from "@/lib/jobs/temporal-fallback";
import type { ProjectAction, ProjectJobSummary, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore, type EditorStore } from "../store/editor-store";
import {
  createGenerationService,
  disposeGenerationRuntime,
  providerUploadConfirmationRequired,
} from "./generation-service";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

const projectDir = "/projects/demo";

function imageRequest(overrides: Partial<MediaGenerationRequest> = {}): MediaGenerationRequest {
  return {
    kind: "generated",
    name: null,
    targetFolderId: null,
    placementIntent: "library",
    prompt: "A brass phonograph",
    model: { provider: "openai", id: "gpt-image-2" },
    references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null },
    settings: { width: 1024, height: 1024, durationSeconds: null, fps: null, aspectRatio: "1:1", resolution: "1024x1024", numImages: 1, generateAudio: null },
    ...overrides,
  };
}

function videoTimelineRequest(): MediaGenerationRequest {
  return imageRequest({
    placementIntent: "timeline",
    model: { provider: "replicate", id: "bytedance/seedance-2.0-fast" },
    settings: { width: 1280, height: 720, durationSeconds: 5, fps: 24, aspectRatio: "16:9", resolution: "720p", generateAudio: false },
  });
}

/**
 * A split project whose backend applies actions locally and keeps the committed copy, so reloads
 * and poller loads see what the editor wrote. Unhandled commands are "unavailable", like a browser.
 */
function setup(handlers: Record<string, Handler> = {}, preferences: Partial<AppSettingsPreferences> = {}) {
  const backend = { project: { ...fixtureProject(), schemaVersion: 2 } as VideoProject };
  const all: Record<string, Handler> = {
    apply_project_actions_to_split_project_folder: (input) => {
      backend.project = applyProjectActionsLocally(backend.project, input.actions as ProjectAction[]);
      return { project: backend.project };
    },
    read_project_snapshot_from_split_project_folder: () => backend.project,
    ...handlers,
  };
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = all[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
  const store = createEditorStore({ projectDir, project: backend.project });
  const batches: ProjectAction[][] = [];
  const apply = store.getState().applyActions;
  store.setState({
    applyActions: (actions, options) => {
      batches.push([...actions]);
      return apply(actions, options);
    },
  });
  const service = createGenerationService(store, { preferences: () => ({ ...defaultAppPreferences, ...preferences }) });
  return { backend, store, batches, service };
}

function calls(command: string) {
  return vi.mocked(backendRequest).mock.calls.filter(([name]) => name === command);
}

/** Completes the queued generation on the "backend" with one mp4 output. */
function completeOnBackend(backend: { project: VideoProject }, assetId: string) {
  const output = { mediaId: "generated-output-1", relativePath: "generated/output-1.mp4", width: 1280, height: 720, durationSeconds: 5, fps: 24 };
  backend.project = {
    ...backend.project,
    media: [...backend.project.media, { id: output.mediaId, name: "Seedance shot", relativePath: output.relativePath, kind: "generated", durationSeconds: 5, width: 1280, height: 720, fps: 24 }],
    generatedAssets: backend.project.generatedAssets.map((asset) => (asset.id === assetId ? { ...asset, status: "completed", outputs: [output] } : asset)),
  };
}

/**
 * What an approved agent bundle records for one generation (Rust `generation_record_actions`): the
 * queued asset, a queued `job-<assetId>` job with its start request, and a timeline placeholder.
 */
function agentGenerationActions(project: VideoProject, assetId: string, request: MediaGenerationRequest): ProjectAction[] {
  const jobId = `job-${assetId}`;
  const createdAt = "2026-09-15T10:00:00.000Z";
  const { kind, ...brief } = request;
  const startRequest = buildFallbackGenerateMediaStartRequest({ projectId: project.id, projectDir, assetId, jobId, mockMode: false, ...brief });
  const placeholder = generatedTimelinePlaceholderAction(project, assetId, request);
  return [
    { type: "recordGeneratedAsset", asset: { id: assetId, kind, ...brief, status: "queued", outputs: [], createdAt, parentAssetId: null, retryOfAssetId: null } },
    { type: "recordJob", job: { id: jobId, kind: "generate_media", status: "queued", updatedAt: createdAt, startRequest } },
    ...(placeholder ? [placeholder] : []),
  ];
}

/** What `EditorRoot` does on unmount: stop the jobs slice polling and the generation runtime. */
function stopRuntime(store: EditorStore) {
  store.getState().stopPolling();
  disposeGenerationRuntime(store);
}

describe("generation service", () => {
  let dispose: (() => void) | null = null;

  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
  });

  afterEach(() => {
    dispose?.();
    dispose = null;
    vi.useRealTimers();
  });

  it("records an in-process image generation without a placeholder and runs it in process", async () => {
    const { backend, store, batches, service } = setup({ run_generate_media_in_process: () => backend.project });
    dispose = () => stopRuntime(store);

    const started = await service.startGeneration(imageRequest());
    if (!started) throw new Error("generation did not start");
    await expect(started.workflow).resolves.toBe(true);

    expect(batches[0]?.map((action) => action.type)).toEqual(["recordJob", "recordGeneratedAsset"]);
    const asset = store.getState().project.generatedAssets.find((candidate) => candidate.id === started.assetId);
    expect(asset).toMatchObject({ status: "queued", prompt: "A brass phonograph", placementIntent: "library", outputs: [], parentAssetId: null, retryOfAssetId: null });
    const [run] = calls("run_generate_media_in_process");
    expect(run?.[1]).toMatchObject({ startRequest: { input: { assetId: started.assetId, jobId: started.assetId, mockMode: false, projectDir } } });
    expect(calls("start_temporal_workflow")).toHaveLength(0);
  });

  it("adds the timeline placeholder in the same batch as the record actions", async () => {
    const { backend, store, batches, service } = setup({ run_generate_media_in_process: () => backend.project });
    dispose = () => stopRuntime(store);

    const started = await service.startGeneration(videoTimelineRequest());
    await started?.workflow;

    expect(batches[0]?.map((action) => action.type)).toEqual(["recordJob", "recordGeneratedAsset", "addItems"]);
    const placeholder = store
      .getState()
      .project.timeline.tracks.flatMap((track) => track.items)
      .find((item) => item.properties.generatedTimelinePlaceholder === true);
    expect(placeholder).toMatchObject({ durationSeconds: 5, label: "Queued generation", source: { type: "generated", artifactId: started?.assetId } });
  });

  it("starts the Temporal workflow when Temporal execution is preferred", async () => {
    const { store, batches, service } = setup(
      {
        start_temporal_workflow: () => ({ status: "started", workflowId: "wf", workflowType: "t", taskQueue: "q", runId: "run-7", message: "" }),
        build_temporal_start_result_action: (input) => ({ type: "updateJobStatus", jobId: (input.job as ProjectJobSummary).id, status: "running", updatedAt: input.updatedAt, runId: input.runId }),
      },
      { generationExecutionBackend: "temporal" },
    );
    dispose = () => stopRuntime(store);

    const started = await service.startGeneration(imageRequest());
    await expect(started?.workflow).resolves.toBe(true);

    expect(calls("run_generate_media_in_process")).toHaveLength(0);
    expect(calls("start_temporal_workflow")[0]?.[1]).toMatchObject({ job: { id: started?.assetId, kind: "generate_media", startRequest: { input: { mockMode: false } } } });
    expect(batches[1]).toEqual([expect.objectContaining({ type: "updateJobStatus", runId: "run-7" })]);
    expect(store.getState().project.jobs.find((job) => job.id === started?.assetId)?.status).toBe("running");
  });

  it.each([
    ["returns unavailable", () => ({ status: "unavailable", workflowId: "wf", workflowType: "t", taskQueue: "q", runId: null, message: "Temporal is not running." }), "Temporal is not running."],
    [
      "throws",
      () => {
        throw new Error("Connection refused");
      },
      "Connection refused",
    ],
  ])("fails the generation job and its asset when the Temporal start %s", async (_case, start, message) => {
    const { store, service } = setup({ start_temporal_workflow: start }, { generationExecutionBackend: "temporal" });
    dispose = () => stopRuntime(store);

    const started = await service.startGeneration(imageRequest());
    await expect(started?.workflow).resolves.toBe(false);

    const { project } = store.getState();
    expect(project.jobs.find((job) => job.id === started?.assetId)).toMatchObject({ status: "failed", failureReason: "The workflow never started." });
    expect(project.generatedAssets.find((asset) => asset.id === started?.assetId)?.status).toBe("failed");
    expect(store.getState().lastError).toContain(message);
  });

  it("polls the project and places completed timeline output over the placeholder", async () => {
    vi.useFakeTimers();
    const { backend, store, batches, service } = setup({ run_generate_media_in_process: () => new Promise(() => undefined) });
    dispose = () => stopRuntime(store);

    const started = await service.startGeneration(videoTimelineRequest());
    if (!started) throw new Error("generation did not start");
    await vi.advanceTimersByTimeAsync(0);
    completeOnBackend(backend, started.assetId);
    await vi.advanceTimersByTimeAsync(1_000);

    const items = store.getState().project.timeline.tracks.flatMap((track) => track.items);
    expect(items.some((item) => item.properties.generatedTimelinePlaceholder === true)).toBe(false);
    expect(items.find((item) => item.source.type === "media" && item.source.mediaId === "generated-output-1")).toMatchObject({ startSeconds: expect.any(Number), durationSeconds: 5 });
    expect(batches.at(-1)?.map((action) => action.type)).toEqual(["removeItems", "addItems"]);

    // Later polls never place the same output twice.
    await vi.advanceTimersByTimeAsync(5_000);
    expect(batches.filter((batch) => batch.some((action) => action.type === "removeItems"))).toHaveLength(1);
  });

  it("polls through the jobs slice, never with a second poller", async () => {
    vi.useFakeTimers();
    const { store, service } = setup({ run_generate_media_in_process: () => new Promise(() => undefined) });
    dispose = () => stopRuntime(store);
    store.getState().startPolling();

    await service.startGeneration(imageRequest());
    await service.startGeneration(imageRequest());
    await vi.advanceTimersByTimeAsync(1_000);
    expect(calls("read_project_snapshot_from_split_project_folder")).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(calls("read_project_snapshot_from_split_project_folder")).toHaveLength(2);
  });

  it("cancels a real generation in process and asks fal.ai to cancel the provider request", async () => {
    const { backend, store, service } = setup({
      cancel_generate_media_in_process: () => ({ outcome: "cancelled", project: backend.project }),
      cancel_generate_media_provider_request_in_split_project_folder: () => {
        throw new Error("provider offline");
      },
    });
    const job: ProjectJobSummary = {
      id: "generated-1",
      kind: "generate_media",
      status: "running",
      updatedAt: "2026-09-14T00:00:00.000Z",
      startRequest: { workflowId: "wf", workflowType: "t", taskQueue: "q", input: { mockMode: false }, searchAttributes: {}, activityTypes: [], idReusePolicy: "rejectDuplicate" },
      providerRequest: { provider: "fal.ai", requestId: "r", statusUrl: "s", responseUrl: "o", cancelUrl: "https://fal.run/cancel", submittedAt: "2026-09-14T00:00:00.000Z" },
    };
    backend.project = { ...backend.project, jobs: [...backend.project.jobs, job] };
    store.getState().replaceProject(backend.project);

    await expect(service.cancelGeneration("generated-1")).resolves.toBe(false);

    expect(calls("cancel_generate_media_in_process")[0]?.[1]).toMatchObject({ projectDir, jobId: "generated-1" });
    expect(calls("cancel_generate_media_provider_request_in_split_project_folder")[0]?.[1]).toEqual({ projectDir, jobId: "generated-1" });
    expect(store.getState().lastError).toBe("Generation was cancelled locally, but the provider cancellation request failed: provider offline");
  });

  it("reloads the failed generation when the in-process run rejects", async () => {
    const { backend, store, service } = setup({
      run_generate_media_in_process: (input) => {
        const assetId = (input.startRequest as { input: { assetId: string } }).input.assetId;
        backend.project = { ...backend.project, generatedAssets: backend.project.generatedAssets.map((asset) => (asset.id === assetId ? { ...asset, status: "failed" } : asset)) };
        throw new Error("OpenAI rejected the request");
      },
    });
    dispose = () => stopRuntime(store);

    const started = await service.startGeneration(imageRequest());
    await expect(started?.workflow).resolves.toBe(false);

    expect(store.getState().project.generatedAssets.find((asset) => asset.id === started?.assetId)?.status).toBe("failed");
    expect(store.getState().lastError).toBe("OpenAI rejected the request");
  });

  it("explains why a download cannot be retried or an asset rerun", async () => {
    const { store, service } = setup();
    await expect(service.retryDownload("missing")).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Generated asset missing was not found.");
    await expect(service.retryDownload("sample-generated-shot", "other")).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Generated asset sample-generated-shot has no output media other.");
    await expect(service.retryDownload("sample-generated-shot")).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Generated asset sample-generated-shot has no retriable output download.");
    await expect(service.retryDownload("sample-generated-shot", "sample-generated-output")).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Generated asset sample-generated-shot output sample-generated-output has no provider retry URL. Rerun generation to recreate the file.");
    await expect(service.rerun("missing")).resolves.toBeNull();
    expect(store.getState().lastError).toBe("Generated asset missing cannot be rerun without a prompt.");
  });

  it("queues one variation of the original prompt under the original generation", async () => {
    const { backend, store, batches, service } = setup({ run_generate_media_in_process: () => backend.project });
    dispose = () => stopRuntime(store);

    const started = await service.createVariations("sample-generated-shot", 1);
    await started?.[0]?.workflow;

    expect(started).toHaveLength(1);
    expect(batches[0]?.map((action) => action.type)).toEqual(["recordJob", "recordGeneratedAsset"]);
    const variation = store.getState().project.generatedAssets.find((asset) => asset.id === started?.[0]?.assetId);
    expect(variation).toMatchObject({ name: "Bundled Edison restoration", prompt: "Restore the public-domain newsreel with a warmer high-contrast treatment", status: "queued", parentAssetId: "sample-generated-shot", retryOfAssetId: "sample-generated-shot" });
  });

  it("records a variation set in one batch and starts the runs one after another", async () => {
    const releases: (() => void)[] = [];
    const { backend, store, batches, service } = setup({
      run_generate_media_in_process: () => new Promise((resolve) => releases.push(() => resolve(backend.project))),
    });
    dispose = () => stopRuntime(store);

    const started = await service.createVariations("sample-generated-shot", 2);
    if (!started) throw new Error("variations did not start");

    expect(batches).toHaveLength(1);
    expect(batches[0]?.map((action) => action.type)).toEqual(["recordJob", "recordGeneratedAsset", "recordJob", "recordGeneratedAsset"]);
    const names = started.map((run) => store.getState().project.generatedAssets.find((asset) => asset.id === run.assetId)?.name);
    expect(names).toEqual(["Storm Clouds", "Radiant Backlight"]);
    await vi.waitFor(() => expect(calls("run_generate_media_in_process")).toHaveLength(1));
    releases[0]?.();
    await started[0]?.workflow;
    await vi.waitFor(() => expect(calls("run_generate_media_in_process")).toHaveLength(2));
    releases[1]?.();
    await expect(started[1]?.workflow).resolves.toBe(true);
  });

  it("starts an agent bundle's recorded jobs one after another, skipping jobs no longer queued when their turn comes", async () => {
    const releases: (() => void)[] = [];
    const { backend, store, batches, service } = setup({
      run_generate_media_in_process: () => new Promise((resolve) => releases.push(() => resolve(backend.project))),
    });
    dispose = () => stopRuntime(store);
    backend.project = applyProjectActionsLocally(backend.project, [
      ...agentGenerationActions(backend.project, "agent-shot-1", imageRequest()),
      ...agentGenerationActions(backend.project, "agent-shot-2", imageRequest()),
      ...agentGenerationActions(backend.project, "agent-shot-3", imageRequest()),
    ]);
    backend.project = { ...backend.project, jobs: backend.project.jobs.map((job) => (job.id === "job-agent-shot-3" ? { ...job, status: "running" } : job)) };
    store.getState().replaceProject(backend.project);

    const started = service.startRecorded(["job-agent-shot-1", "job-agent-shot-2", "job-agent-shot-3", "job-missing"]);

    expect(started.map((run) => run.assetId)).toEqual(["agent-shot-1", "agent-shot-2"]);
    expect(batches).toEqual([]);
    await vi.waitFor(() => expect(calls("run_generate_media_in_process")).toHaveLength(1));
    expect(calls("run_generate_media_in_process")[0]?.[1]).toMatchObject({ startRequest: { input: { assetId: "agent-shot-1", jobId: "job-agent-shot-1" } } });
    // An Undo drops the second job before the first run settles, so it never starts.
    store.getState().replaceProject({ ...store.getState().project, jobs: store.getState().project.jobs.filter((job) => job.id !== "job-agent-shot-2") });
    backend.project = store.getState().project;
    releases[0]?.();
    await expect(started[0]?.workflow).resolves.toBe(true);
    await expect(started[1]?.workflow).resolves.toBe(true);
    expect(calls("run_generate_media_in_process")).toHaveLength(1);
  });

  it("places a started agent generation's output over its placeholder by the start request's asset id", async () => {
    vi.useFakeTimers();
    const { backend, store, batches, service } = setup({ run_generate_media_in_process: () => new Promise(() => undefined) });
    dispose = () => stopRuntime(store);
    backend.project = applyProjectActionsLocally(backend.project, agentGenerationActions(backend.project, "agent-shot-1", videoTimelineRequest()));
    store.getState().replaceProject(backend.project);

    const [started] = service.startRecorded(["job-agent-shot-1"]);
    expect(started?.assetId).toBe("agent-shot-1");
    await vi.advanceTimersByTimeAsync(0);
    completeOnBackend(backend, "agent-shot-1");
    await vi.advanceTimersByTimeAsync(1_000);

    const items = store.getState().project.timeline.tracks.flatMap((track) => track.items);
    expect(items.some((item) => item.properties.generatedTimelinePlaceholder === true)).toBe(false);
    expect(items.some((item) => item.source.type === "media" && item.source.mediaId === "generated-output-1")).toBe(true);
    expect(batches[batches.length - 1]?.map((action) => action.type)).toEqual(["removeItems", "addItems"]);
  });

  it("forgets a generation an agent Undo removed: its output is never placed and its failed run reports nothing", async () => {
    vi.useFakeTimers();
    const pending: { fail: (() => void) | null } = { fail: null };
    const { backend, store, batches, service } = setup({
      run_generate_media_in_process: () =>
        new Promise((_, reject) => {
          pending.fail = () => reject("generated asset reference is missing: agent-shot-1");
        }),
    });
    dispose = () => stopRuntime(store);
    const before = backend.project;
    backend.project = applyProjectActionsLocally(backend.project, agentGenerationActions(backend.project, "agent-shot-1", videoTimelineRequest()));
    store.getState().replaceProject(backend.project);
    const [started] = service.startRecorded(["job-agent-shot-1"]);
    await vi.advanceTimersByTimeAsync(0);

    service.forgetRemoved(["agent-shot-1"]);
    backend.project = before;
    store.getState().replaceProject(before);
    pending.fail?.();

    await expect(started?.workflow).resolves.toBe(false);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(store.getState().lastError).toBeNull();
    expect(store.getState().project.generatedAssets).toEqual(before.generatedAssets);
    expect(batches).toEqual([]);
  });

  it("blocks variations without a prompt", async () => {
    const { store, service } = setup();
    store.getState().replaceProject({ ...store.getState().project, generatedAssets: store.getState().project.generatedAssets.map((asset) => ({ ...asset, prompt: " " })) });
    await expect(service.createVariations("sample-generated-shot", 1)).resolves.toBeNull();
    expect(store.getState().lastError).toBe("This generation has no prompt to vary.");
  });

  it("queues an upscale of the clip's source span into the library", async () => {
    const { backend, store, batches, service } = setup({ run_generate_media_in_process: () => backend.project });
    dispose = () => stopRuntime(store);

    const started = await service.upscale("media-1", { itemId: "item-1", sourceIn: 1, sourceOut: 3 });
    await started?.workflow;

    expect(batches[0]?.map((action) => action.type)).toEqual(["recordJob", "recordGeneratedAsset"]);
    expect(store.getState().project.generatedAssets.find((asset) => asset.id === started?.assetId)).toMatchObject({
      placementIntent: "library",
      model: { id: "fal-ai/video-upscaler" },
      references: { mediaIds: ["media-1"], sourceVideoMediaRef: "media-1" },
      settings: { videoSourceStartSeconds: 1, videoSourceEndSeconds: 3 },
    });
    await expect(service.upscale("media-voiceover")).resolves.toBeNull();
    expect(store.getState().lastError).toBe("Only images and videos can be upscaled.");
  });

  it("queues music from a video clip onto the timeline", async () => {
    const { backend, store, service } = setup({ run_generate_media_in_process: () => backend.project });
    dispose = () => stopRuntime(store);

    const started = await service.videoToAudio("media-1", "music", { itemId: "item-1", timelineStartSeconds: 0, durationSeconds: 4 });
    await started?.workflow;

    expect(store.getState().project.generatedAssets.find((asset) => asset.id === started?.assetId)).toMatchObject({ kind: "audio", placementIntent: "timeline", name: "Generated music" });
    await expect(service.videoToAudio("media-voiceover", "sfx", { itemId: "music-bed", timelineStartSeconds: 0, durationSeconds: 4 })).resolves.toBeNull();
    expect(store.getState().lastError).toBe("Music and sound effects need a video clip.");
  });

  it("swaps a completed generated output into a clip, or explains why not", async () => {
    const { store, batches, service } = setup({
      apply_project_actions_to_split_project_folder: () => ({ project: store.getState().project }),
    });
    await expect(service.replaceWithOutput("item-1", "sample-generated-output")).resolves.toBe(true);
    expect(batches).toEqual([[{ type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId: "item-1", mediaId: "sample-generated-output" } }]]);

    await expect(service.replaceWithOutput("music-bed", "sample-generated-output")).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Audio clips can't swap generated outputs yet.");
    expect(batches).toHaveLength(1);
  });

  it("asks for provider upload confirmation only for referenced media when the setting is on", () => {
    const referenced = imageRequest({ references: { mediaIds: ["media-1"], firstFrameMediaId: null, lastFrameMediaId: null } });
    expect(providerUploadConfirmationRequired(referenced, defaultAppPreferences)).toBe(true);
    expect(providerUploadConfirmationRequired(imageRequest(), defaultAppPreferences)).toBe(false);
    expect(providerUploadConfirmationRequired(referenced, { ...defaultAppPreferences, requireProviderUploadConfirmation: false })).toBe(false);
  });
});
