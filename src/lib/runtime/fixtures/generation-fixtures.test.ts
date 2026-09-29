import { describe, expect, it } from "vitest";
import { defaultAppPreferences } from "@/lib/app-settings";
import { generationModelCatalogFromPayload } from "@/lib/generation/catalog";
import type {
  CancelInProcessGenerationResult,
  GenerationModelCatalogPayload,
  ProjectActionWriteResult,
  ProjectJobSummary,
  TemporalWorkflowStartRequest,
  VideoProject,
} from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { createFixtureProjectStore } from "./fixture-project-store";
import { generationFixtureOperations, generationFixturePolls } from "./generation-fixtures";
import { projectFixtureOperations } from "./project-fixtures";
import { speechFixtureOperations } from "./speech-fixtures";

const projectDir = "/tmp/video-creater-editor-project";
const createdAt = "2026-09-15T10:00:00.000Z";

function setup() {
  const store = createFixtureProjectStore();
  const handlers = new Map([...projectFixtureOperations(store), ...speechFixtureOperations(store), ...generationFixtureOperations(store)]);
  function request<Result>(operation: string, input: Record<string, unknown> = {}): Promise<Result> {
    const handler = handlers.get(operation);
    if (!handler) return Promise.reject(new Error(`no generation fixture handler for ${operation}`));
    return new Promise((resolve) => resolve(handler(structuredClone(input)))).then((result) => structuredClone(result) as Result);
  }
  const reload = () => request<VideoProject>("load_split_project_from_folder", { projectDir });

  /** The generation service's queue(): record the job and the queued asset, then build the start request. */
  async function queue(assetId: string, mockMode: boolean): Promise<TemporalWorkflowStartRequest> {
    const job = await request<ProjectJobSummary>("build_temporal_job_summary", { kind: "generate_media", projectId: "project-sample", jobId: assetId, status: "queued", updatedAt: createdAt });
    const brief = {
      name: "Lab bench wide shot",
      targetFolderId: null,
      placementIntent: "library",
      prompt: "A lab bench in morning light",
      model: mockMode ? { provider: "mock", id: "fixture-video" } : { provider: "replicate", id: "bytedance/seedance-1-pro-fast" },
      references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null },
      settings: { width: 1920, height: 1080, durationSeconds: 4, fps: 24, aspectRatio: "16:9" },
    };
    const startRequest = await request<TemporalWorkflowStartRequest>("build_temporal_generate_media_start_request", { projectId: "project-sample", projectDir, assetId, jobId: assetId, mockMode, ...brief });
    await request("apply_project_actions_to_split_project_folder", {
      projectDir,
      actions: [
        { type: "recordJob", job: { ...job, startRequest } },
        { type: "recordGeneratedAsset", asset: { id: assetId, kind: "video", status: "queued", outputs: [], createdAt, parentAssetId: null, retryOfAssetId: null, ...brief } },
      ],
    });
    return startRequest;
  }

  return { request, reload, queue };
}

function asset(project: VideoProject, id: string) {
  return project.generatedAssets.find((candidate) => candidate.id === id);
}

describe("generation fixture operations", () => {
  it("lists mock models that are always enabled and a live model behind preferences", async () => {
    const { request } = setup();
    const payload = await request<GenerationModelCatalogPayload>("list_generation_model_catalog");
    const catalog = generationModelCatalogFromPayload(payload, defaultAppPreferences);
    expect(catalog?.image?.map((model) => model.id)).toEqual(["fixture-image"]);
    expect(catalog?.video?.map((model) => model.id)).toEqual(["fixture-video"]);
    const enabled = generationModelCatalogFromPayload(payload, { ...defaultAppPreferences, enabledGenerationModelIds: ["replicate:bytedance/seedance-1-pro-fast"] });
    expect(enabled?.video?.map((model) => model.id)).toEqual(["fixture-video", "bytedance/seedance-1-pro-fast"]);
  });

  it("runs an in-process generation until the second folder reload, completing it with an output", async () => {
    const { request, reload, queue } = setup();
    await request("save_split_project_to_folder", { projectDir, project: createSampleProject(), expectedRevision: 0 });
    const startRequest = await queue("generated-shot-1", false);

    let settled: VideoProject | null = null;
    const run = request<VideoProject>("run_generate_media_in_process", { startRequest, updatedAt: createdAt }).then((project) => (settled = project));
    await Promise.resolve();
    const running = await reload();
    expect(asset(running, "generated-shot-1")?.status).toBe("running");
    expect(running.jobs.find((job) => job.id === "generated-shot-1")?.status).toBe("running");
    expect(generationFixturePolls).toBe(2);
    expect(settled).toBeNull();

    const completed = await reload();
    await run;
    expect(settled).toEqual(completed);
    expect(asset(completed, "generated-shot-1")).toMatchObject({
      status: "completed",
      outputs: [{ mediaId: "generated-shot-1-mock-output", relativePath: "generated/generated-shot-1/mock-output.mp4", width: 1920, height: 1080, durationSeconds: 4, fps: 24 }],
    });
    expect(completed.media.find((media) => media.id === "generated-shot-1-mock-output")).toMatchObject({ kind: "generated", durationSeconds: 4 });
    expect(completed.jobs.find((job) => job.id === "generated-shot-1")?.status).toBe("completed");
  });

  it("cancels a waiting generation, settling the run with the cancelled job and asset", async () => {
    const { request, reload, queue } = setup();
    await request("save_split_project_to_folder", { projectDir, project: createSampleProject(), expectedRevision: 0 });
    const startRequest = await queue("generated-shot-2", false);
    const run = request<VideoProject>("run_generate_media_in_process", { startRequest, updatedAt: createdAt });
    await Promise.resolve();

    const cancel = await request<CancelInProcessGenerationResult>("cancel_generate_media_in_process", { projectDir, jobId: "generated-shot-2", updatedAt: createdAt });
    expect(cancel.outcome).toBe("cancelled");
    expect(asset(cancel.project, "generated-shot-2")?.status).toBe("cancelled");
    expect(cancel.project.jobs.find((job) => job.id === "generated-shot-2")?.status).toBe("cancelled");
    await expect(run).resolves.toEqual(cancel.project);
    await reload();
    await expect(reload()).resolves.toMatchObject({ generatedAssets: expect.arrayContaining([expect.objectContaining({ id: "generated-shot-2", status: "cancelled", outputs: [] })]) });
    await expect(request<CancelInProcessGenerationResult>("cancel_generate_media_in_process", { projectDir, jobId: "generated-shot-2", updatedAt: createdAt })).resolves.toMatchObject({
      outcome: "alreadyCancelled",
    });
  });

  it("completes a queued mock generation at once as a content write, and only once", async () => {
    const { request, queue } = setup();
    await request("save_split_project_to_folder", { projectDir, project: createSampleProject(), expectedRevision: 0 });
    await queue("generated-mock-1", true);
    const result = await request<ProjectActionWriteResult>("complete_mock_generated_asset_in_split_project_folder", { projectDir, assetId: "generated-mock-1", updatedAt: createdAt, replacementItemId: null });
    expect(asset(result.project, "generated-mock-1")?.outputs).toHaveLength(1);
    expect(result.project.contentRevision).toBe(3);
    await expect(request("complete_mock_generated_asset_in_split_project_folder", { projectDir, assetId: "generated-mock-1", updatedAt: createdAt })).rejects.toBe(
      "generated asset cannot be completed by mock worker: generated-mock-1",
    );
  });
});
