import { generationJobAssetId } from "../../jobs/activity-records";
import { buildFallbackGenerateMediaStartRequest } from "../../jobs/temporal-fallback";
import type {
  CancelInProcessGenerationResult,
  GeneratedAsset,
  GenerationModelCatalogPayload,
  GenerationModelUiCapabilities,
  ProjectAction,
  ProjectActionWriteResult,
  ProjectJobSummary,
  TemporalWorkflowStartRequest,
  VideoProject,
} from "../../project";
import type { FixtureOperationHandler } from "../adapters/fixture-transport";
import { fixtureWriteReport, type FixtureProjectStore } from "./fixture-project-store";

/**
 * DEV-only media generation handlers over the shared fixture project store:
 *
 * - A model catalog with always-enabled mock image and video models and one live video model.
 * - `run_generate_media_in_process` marks the job and asset running, then completes on the store's
 *   job clock: the second folder reload records a fixture output (and its media) and resolves the run
 *   with the project, as the native runner does. Mock-mode requests complete at once.
 * - Cancel marks a waiting run's job and asset cancelled and resolves the run with that project.
 * - `complete_mock_generated_asset_in_split_project_folder` completes a queued or running asset (the
 *   Media tab's "Complete mock", or the AI tab's queued lab shots) as one content write.
 *
 * The editor places timeline output itself once the completed asset merges, as in the desktop app.
 */

/** Folder reloads an in-process generation takes before it completes. */
export const generationFixturePolls = 2;

const videoCapabilities: GenerationModelUiCapabilities = {
  durations: [4, 8],
  resolutions: ["720p", "1080p"],
  aspectRatios: ["16:9", "9:16", "1:1"],
  supportsFirstFrame: true,
  supportsLastFrame: false,
  maxReferenceImages: 0,
  maxReferenceVideos: 0,
  maxReferenceAudios: 0,
  maxTotalReferences: null,
  maxCombinedVideoRefSeconds: null,
  maxCombinedAudioRefSeconds: null,
  framesAndReferencesExclusive: false,
  referenceTagNoun: "reference",
  requiresSourceVideo: false,
  requiresReferenceImage: false,
};

const imageCapabilities: GenerationModelUiCapabilities = {
  resolutions: null,
  aspectRatios: ["16:9", "1:1", "9:16"],
  qualities: null,
  supportsImageReference: false,
  maxImages: 1,
};

function catalog(): GenerationModelCatalogPayload {
  const model = (provider: string, id: string, kind: "image" | "video", displayName: string) => ({
    provider,
    id,
    kind,
    displayName,
    allowedEndpoints: [id],
    responseShape: kind === "image" ? ("images" as const) : ("video" as const),
    cancellationCapability: "local" as const,
    uiCapabilities: kind === "image" ? imageCapabilities : videoCapabilities,
    paidOnly: provider !== "mock",
  });
  return {
    loaded: true,
    generationCatalog: {
      source: "builtin",
      catalogVersion: "fixture",
      capabilitiesVersion: "fixture",
      stale: false,
      cacheStatus: "fresh",
      hashVerified: true,
      signatureVerified: true,
      remoteConfigured: false,
    },
    generationModels: [
      model("mock", "fixture-image", "image", "Fixture image"),
      model("mock", "fixture-video", "video", "Fixture video"),
      model("replicate", "bytedance/seedance-1-pro-fast", "video", "Seedance 1 Pro Fast"),
    ],
    providerCredentialsExposed: false,
  };
}

function outputExtension(asset: GeneratedAsset): string {
  if (asset.kind === "image") return "png";
  return asset.kind === "audio" ? "mp3" : "mp4";
}

/**
 * The mock worker's output: `<asset>-mock-output`, stored with its media (recorded as Rust
 * `completeGeneratedAsset` records it) and the completed asset and job.
 */
function completed(project: VideoProject, asset: GeneratedAsset, jobUpdatedAt: string): VideoProject {
  const mediaId = `${asset.id}-mock-output`;
  const audio = asset.kind === "audio";
  const output = {
    mediaId,
    relativePath: `generated/${asset.id}/mock-output.${outputExtension(asset)}`,
    width: audio ? 0 : asset.settings.width ?? 1280,
    height: audio ? 0 : asset.settings.height ?? 720,
    durationSeconds: asset.settings.durationSeconds ?? 4,
    fps: audio ? 0 : asset.settings.fps ?? 24,
  };
  const media = {
    id: mediaId,
    name: null,
    relativePath: output.relativePath,
    kind: "generated" as const,
    durationSeconds: output.durationSeconds,
    width: output.width,
    height: output.height,
    fps: output.fps > 0 ? output.fps : null,
    folderId: asset.targetFolderId ?? null,
  };
  return {
    ...project,
    media: [...project.media.filter((candidate) => candidate.id !== mediaId), media],
    generatedAssets: project.generatedAssets.map((candidate) => (candidate.id === asset.id ? { ...candidate, status: "completed", outputs: [output] } : candidate)),
    jobs: project.jobs.map((job) => (job.kind === "generate_media" && generationJobAssetId(job) === asset.id ? { ...job, status: "completed", updatedAt: jobUpdatedAt } : job)),
  };
}

function withStatuses(project: VideoProject, jobId: string, assetId: string, status: "running" | "cancelled", updatedAt: string, runId?: string): VideoProject {
  return {
    ...project,
    jobs: project.jobs.map((job) => (job.id === jobId ? { ...job, status, updatedAt, ...(runId && job.workflow ? { workflow: { ...job.workflow, runId } } : {}) } : job)),
    generatedAssets: project.generatedAssets.map((asset) => (asset.id === assetId ? { ...asset, status } : asset)),
  };
}

function pendingAsset(project: VideoProject, assetId: string): GeneratedAsset {
  const asset = project.generatedAssets.find((candidate) => candidate.id === assetId);
  if (!asset) throw `generated asset was not found: ${assetId}`;
  if (asset.status !== "queued" && asset.status !== "running") throw `generated asset cannot be completed by mock worker: ${assetId}`;
  return asset;
}

interface PendingGeneration {
  readonly jobId: string;
  readonly assetId: string;
  polls: number;
  resolve(project: VideoProject): void;
}

export function generationFixtureOperations(store: FixtureProjectStore): ReadonlyMap<string, FixtureOperationHandler> {
  const pending = new Map<string, PendingGeneration>();

  /** One folder reload: each waiting generation runs, then records its output and resolves. */
  store.onReload(() => {
    for (const run of [...pending.values()]) {
      run.polls += 1;
      if (run.polls < generationFixturePolls) continue;
      pending.delete(run.jobId);
      const project = store.require();
      const asset = project.generatedAssets.find((candidate) => candidate.id === run.assetId);
      run.resolve(asset ? store.record(completed(project, asset, new Date().toISOString())) : project);
    }
  });

  function runInProcess(input: Record<string, unknown>): VideoProject | Promise<VideoProject> {
    const { startRequest, updatedAt } = input as { startRequest: TemporalWorkflowStartRequest; updatedAt: string };
    const { projectId, jobId, assetId, mockMode } = startRequest.input as { projectId: string; jobId: string; assetId: string; mockMode?: boolean };
    const project = store.require();
    if (project.id !== projectId) throw "generation start request projectId does not match project folder";
    const job = project.jobs.find((candidate) => candidate.id === jobId);
    if (!job) throw `generation job was not found: ${jobId}`;
    const asset = project.generatedAssets.find((candidate) => candidate.id === assetId);
    if (!asset) throw `generation asset was not found or has no provider: ${assetId}`;
    if (job.status === "cancelled" || asset.status === "cancelled") return project;
    const running = store.record(withStatuses(project, jobId, assetId, "running", updatedAt, `in-process/${startRequest.workflowId}`));
    if (mockMode === true) return store.record(completed(running, pendingAsset(running, assetId), updatedAt));
    return new Promise<VideoProject>((resolve) => {
      pending.set(jobId, { jobId, assetId, polls: 0, resolve });
    });
  }

  function cancelInProcess(input: Record<string, unknown>): CancelInProcessGenerationResult {
    const { jobId, updatedAt } = input as { jobId: string; updatedAt: string };
    const project = store.require();
    const job = project.jobs.find((candidate) => candidate.id === jobId);
    if (!job) throw `job was not found: ${jobId}`;
    const asset = project.generatedAssets.find((candidate) => candidate.id === job.startRequest?.input.assetId);
    if (!asset) throw `generation asset was not found: ${jobId}`;
    if (job.status === "cancelled" || asset.status === "cancelled") return { outcome: "alreadyCancelled", project };
    if (job.status === "completed" || job.status === "failed" || asset.status === "completed" || asset.status === "failed") return { outcome: "alreadyTerminal", project };
    const cancelled = store.record(withStatuses(project, jobId, asset.id, "cancelled", updatedAt));
    const run = pending.get(jobId);
    pending.delete(jobId);
    run?.resolve(cancelled);
    return { outcome: "cancelled", project: cancelled };
  }

  return new Map<string, FixtureOperationHandler>([
    ["list_generation_model_catalog", catalog],
    ["build_temporal_generate_media_start_request", (input) => buildFallbackGenerateMediaStartRequest(input as unknown as Parameters<typeof buildFallbackGenerateMediaStartRequest>[0])],
    ["run_generate_media_in_process", runInProcess],
    ["cancel_generate_media_in_process", cancelInProcess],
    [
      "build_temporal_generate_media_failure_actions",
      (input) => {
        const { job, assetId, runId = null, updatedAt } = input as { job: ProjectJobSummary; assetId: string; runId?: string | null; updatedAt: string };
        const actions: ProjectAction[] = [
          { type: "updateJobStatus", jobId: job.id, status: "failed", updatedAt, runId },
          { type: "updateGeneratedAssetStatus", assetId, status: "failed" },
        ];
        return actions;
      },
    ],
    [
      "complete_mock_generated_asset_in_split_project_folder",
      (input) => {
        const { assetId, updatedAt } = input as { assetId: string; updatedAt: string };
        const project = store.require();
        const result: ProjectActionWriteResult = { project: store.write(completed(project, pendingAsset(project, assetId), updatedAt)), report: fixtureWriteReport() };
        return result;
      },
    ],
  ]);
}

