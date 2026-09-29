import { useMemo } from "react";
import {
  generationExecutionModeForModel,
  loadAppSettingsPreferences,
  type AppSettingsPreferences,
} from "@/lib/app-settings";
import { generationModelCatalogFromPayload } from "@/lib/generation/catalog";
import { generatedOutputReplacementBlocker, upscaleRequestPlan, variationDraftsForAsset, videoAudioRequestPlan, type RequestPlan } from "@/lib/generation/clip-actions";
import { generatedMediaAssetId, generatedVariationId, generatedVariationSetId } from "@/lib/generation/requests";
import {
  generatedAssetPlacementIntent,
  generatedOutputTimelineActions,
  generatedTimelinePlaceholderAction,
} from "@/lib/generation/timeline-placement";
import type {
  GenerationModelCatalog,
  MediaGenerationRequest,
  SourceClipUpscaleContext,
  SourceClipVideoAudioContext,
  SourceClipVideoAudioKind,
} from "@/lib/generation/types";
import { generationJobAssetId } from "@/lib/jobs/activity-records";
import {
  buildFallbackGenerateMediaStartRequest,
  buildTemporalJobSummary,
} from "@/lib/jobs/temporal-fallback";
import {
  buildTemporalGenerateMediaFailureActions,
  buildTemporalGenerateMediaStartRequest,
  cancelGenerateMediaInProcess,
  cancelGenerateMediaProviderRequestInSplitProjectFolder,
  completeMockGeneratedAssetInSplitProjectFolder,
  listGenerationModelCatalog,
  loadSplitProjectFromFolder,
  retryGeneratedAssetOutputDownloadInSplitProjectFolder,
  runGenerateMediaInProcess,
  type GeneratedAsset,
  type ProjectAction,
  type ProjectJobSummary,
  type TemporalGenerateMediaBrief,
  type TemporalWorkflowStartRequest,
  type VideoProject,
} from "@/lib/project";
import { BackendOperationError, isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { EditorStore } from "../store/editor-store";
import { useEditorStoreApi } from "../store/editor-store-context";
import { createGenerationWorkflowStarts, isRealGeneration, usesSplitFolder } from "./generation-workflow-start";

/** A queued generation; `workflow` settles when the in-process run, Temporal start or mock start finishes. */
interface GenerationStart {
  readonly assetId: string;
  readonly workflow: Promise<boolean>;
}

/**
 * Store-bound media generation, following the pre-cut request lifecycle: record the job and the
 * queued asset (with a timeline placeholder when requested) as one batch, start the workflow
 * (mock, Temporal or in-process by preference), then place timeline output once the jobs slice's
 * polling (or the run's own result) merges the completed asset.
 * Methods resolve `false`/`null` when blocked or failed, with the reason in `lastError`.
 */
export interface GenerationService {
  /** Pre-cut `queueMediaGeneration`, minus the upload confirmation (the Generate view asks first). */
  startGeneration(request: MediaGenerationRequest): Promise<GenerationStart | null>;
  /** Pre-cut `rerunGeneratedAsset`: queues a retry of the asset with its original prompt and settings. */
  rerun(assetId: string): Promise<GenerationStart | null>;
  /**
   * One variation of the original prompt (pre-cut `queueGeneratedClipVariation`), or for 2+ a variation
   * set of default drafts (`queueGeneratedClipVariationSet`): every run recorded in one batch, then
   * started one after another.
   */
  createVariations(assetId: string, count: number): Promise<readonly GenerationStart[] | null>;
  /** Pre-cut "replace clip with generated output" (`replaceTimelineItemWithGeneratedOutput`). */
  replaceWithOutput(itemId: string, mediaId: string): Promise<boolean>;
  /** Pre-cut `queueMediaUpscale`; `context` limits a video to the clip's source span. */
  upscale(mediaId: string, context?: SourceClipUpscaleContext): Promise<GenerationStart | null>;
  /** Pre-cut `queueVideoAudioGeneration`: music or sound effects placed under the video clip. */
  videoToAudio(mediaId: string, kind: SourceClipVideoAudioKind, context: SourceClipVideoAudioContext): Promise<GenerationStart | null>;
  /**
   * Starts generations already recorded as queued jobs with start requests (an approved agent bundle
   * records them), one after another. A job no longer queued when its turn comes is skipped.
   */
  startRecorded(jobIds: readonly string[]): readonly GenerationStart[];
  /**
   * Stops following generations an agent Undo removed (the backend cancelled the running ones): their
   * output is never placed, and a run that fails because its job is gone reports nothing.
   */
  forgetRemoved(assetIds: readonly string[]): void;
  /** Pre-cut `failMockGeneration`: cancels a real run (and its provider request), or fails a mock one. */
  cancelGeneration(assetId: string): Promise<boolean>;
  /** Pre-cut `retryGeneratedAssetDownload`. */
  retryDownload(assetId: string, outputMediaId?: string): Promise<boolean>;
  /** Pre-cut `completeMockGeneration` (fixture runtime only), placing timeline output. */
  completeMock(assetId: string): Promise<boolean>;
}

interface GenerationServiceOptions {
  /** Defaults to the accepted app preferences, as the pre-cut editor read them. */
  readonly preferences?: () => AppSettingsPreferences;
}

/** Per-store workflow state shared by every service instance. */
interface GenerationRuntime {
  /** Jobs whose workflow start is in flight, so a double start never runs twice. */
  readonly starting: Set<string>;
  /** Generations started in this session whose timeline output is still to be placed. */
  readonly awaitingOutput: Set<string>;
  /** Watches project changes (polled merges included) to place awaited output; null until tracking starts. */
  unsubscribe: (() => void) | null;
}

const runtimes = new WeakMap<EditorStore, GenerationRuntime>();

const providerUploadModelIds: ReadonlySet<string> = new Set(["sonilo/v1.1/video-to-music", "mirelo-ai/sfx-v1.5/video-to-audio"]);
const providerCancelProviders: ReadonlySet<string> = new Set(["fal.ai", "replicate"]);

export const providerUploadConfirmationCopy =
  "This generation references local project media. Continue and allow provider upload preparation for the referenced media?";

/** Plain reason text: native commands reject with strings, which the backend client wraps as the cause. */
function errorMessage(error: unknown): string {
  const cause: unknown = error instanceof BackendOperationError ? error.cause : error;
  if (typeof cause === "string") return cause;
  return cause instanceof Error ? cause.message : String(cause);
}

function now(): string {
  return new Date().toISOString();
}

function isProject(value: unknown): value is VideoProject {
  return typeof value === "object" && value !== null && "timeline" in value && "generatedAssets" in value;
}

/** The request fields the provider upload check reads, so a generated asset can be checked too. */
type ProviderUploadCheck = Pick<MediaGenerationRequest, "placementIntent" | "model" | "settings"> & {
  readonly references: Pick<MediaGenerationRequest["references"], "mediaIds" | "firstFrameMediaId" | "lastFrameMediaId">;
};

/** Pre-cut `generationReferencesNeedProviderUpload`. */
function referencesNeedProviderUpload(request: ProviderUploadCheck): boolean {
  const { references, settings } = request;
  return (
    references.mediaIds.length > 0 ||
    Boolean(references.firstFrameMediaId) ||
    Boolean(references.lastFrameMediaId) ||
    (request.placementIntent === "timeline" &&
      request.model.provider === "fal.ai" &&
      providerUploadModelIds.has(request.model.id) &&
      typeof settings.videoSourceStartSeconds === "number" &&
      typeof settings.videoSourceEndSeconds === "number")
  );
}

/** Pre-cut `confirmProviderUploadForGeneration` guard: true when the user must confirm before starting. */
export function providerUploadConfirmationRequired(request: ProviderUploadCheck, preferences: AppSettingsPreferences): boolean {
  return preferences.requireProviderUploadConfirmation && referencesNeedProviderUpload(request);
}

/** Pre-cut catalog probe: the enabled generation models, or null when the backend has none loaded. */
export async function loadGenerationCatalog(preferences: AppSettingsPreferences): Promise<GenerationModelCatalog | null> {
  return generationModelCatalogFromPayload(await listGenerationModelCatalog(), preferences);
}

function outputOnTimeline(project: VideoProject, mediaId: string): boolean {
  return project.timeline.tracks.some((track) => track.items.some((item) => item.source.type === "media" && item.source.mediaId === mediaId));
}

type QueuedAsset = Omit<GeneratedAsset, "schemaVersion" | "status" | "outputs" | "createdAt">;

/** Pre-cut variation asset: inherits folder, placement, model, references and settings. */
function variationOf(asset: GeneratedAsset, id: string, name: string | null, prompt: string): QueuedAsset {
  return {
    id,
    kind: asset.kind,
    name,
    targetFolderId: asset.targetFolderId ?? null,
    placementIntent: generatedAssetPlacementIntent(asset),
    prompt,
    model: asset.model,
    references: asset.references,
    settings: asset.settings,
    parentAssetId: asset.parentAssetId ?? asset.id,
    retryOfAssetId: asset.id,
  };
}

function queuedGeneration(project: VideoProject, jobId: string): ProjectJobSummary | null {
  const job = project.jobs.find((candidate) => candidate.id === jobId);
  return job?.kind === "generate_media" && job.status === "queued" && job.startRequest ? job : null;
}

function hasGeneratedAsset(project: VideoProject, assetId: string): boolean {
  return project.generatedAssets.some((asset) => asset.id === assetId);
}

export function createGenerationService(store: EditorStore, options: GenerationServiceOptions = {}): GenerationService {
  const state = () => store.getState();
  const preferences = options.preferences ?? loadAppSettingsPreferences;
  let runtime = runtimes.get(store);
  if (!runtime) {
    runtime = { starting: new Set(), awaitingOutput: new Set(), unsubscribe: null };
    runtimes.set(store, runtime);
  }
  // Every service instance for the store shares this object; disposal only drops its subscription.
  const shared = runtime;

  function block(reason: string): false {
    state().setLastError(reason);
    return false;
  }

  /** Places a completed timeline-targeted output (replacing its placeholder) unless it is already there. */
  async function placeOutput(assetId: string, select: boolean): Promise<void> {
    const { project } = state();
    const asset = project.generatedAssets.find((candidate) => candidate.id === assetId);
    const outputId = asset?.outputs[0]?.mediaId;
    if (!asset || !outputId || asset.status !== "completed" || generatedAssetPlacementIntent(asset) !== "timeline") return;
    if (outputOnTimeline(project, outputId)) return;
    const insertion = generatedOutputTimelineActions(project, outputId);
    if (!insertion) return;
    const applied = await state().applyActions(insertion.actions);
    if (applied && select) state().selectItems([insertion.itemId]);
  }

  /**
   * Places the output of generations this session started once they settle. Only tracked generations
   * are placed, so clips the user removed never come back.
   */
  function placeSettledOutputs(project: VideoProject): void {
    for (const assetId of [...shared.awaitingOutput]) {
      const asset = project.generatedAssets.find((candidate) => candidate.id === assetId);
      if (!asset || asset.status === "queued" || asset.status === "running") continue;
      shared.awaitingOutput.delete(assetId);
      void placeOutput(assetId, false);
    }
  }

  /** Merges a backend project's job state into the store (never over an edit in flight), then places settled output. */
  async function commitProject(project: VideoProject): Promise<void> {
    await state().mergeLoadedProject(project);
    placeSettledOutputs(state().project);
  }

  /**
   * Awaits the job's output. The jobs slice polls the folder while the job is active (one poller per
   * store), and its merges reach `placeSettledOutputs` through this store subscription.
   */
  function track(jobId: string): void {
    shared.awaitingOutput.add(jobId);
    shared.unsubscribe ??= store.subscribe((next, previous) => {
      if (next.project !== previous.project) placeSettledOutputs(next.project);
    });
    state().startPolling();
  }

  /** Pre-cut `buildGenerateMediaJob`. */
  async function buildJob(assetId: string, createdAt: string, brief: TemporalGenerateMediaBrief): Promise<ProjectJobSummary> {
    const { project, projectDir } = state();
    const job = await buildTemporalJobSummary("generate_media", project.id, assetId, createdAt);
    if (projectDir.trim().length === 0) return job;
    const input = {
      projectId: project.id,
      projectDir,
      assetId,
      jobId: assetId,
      mockMode: generationExecutionModeForModel(brief.model) === "mock",
      ...brief,
    };
    let startRequest: TemporalWorkflowStartRequest;
    try {
      startRequest = await buildTemporalGenerateMediaStartRequest(input);
    } catch (error) {
      if (!isBackendUnavailableError(error)) throw error;
      startRequest = buildFallbackGenerateMediaStartRequest(input);
    }
    return { ...job, startRequest };
  }

  const { startMockWorkflow, startTemporal } = createGenerationWorkflowStarts({ store, starting: shared.starting, block, track, errorMessage });

  /** Pre-cut `startGenerateMediaWorkflow`. */
  async function startWorkflow(job: ProjectJobSummary): Promise<boolean> {
    try {
      if (job.startRequest?.input.mockMode === true) return await startMockWorkflow(job);
    } catch (error) {
      return block(errorMessage(error));
    }
    if (preferences().generationExecutionBackend === "temporal") return startTemporal(job);
    if (!job.startRequest || shared.starting.has(job.id)) return true;
    shared.starting.add(job.id);
    try {
      const completion = runGenerateMediaInProcess({ startRequest: job.startRequest, updatedAt: now() });
      const assetId = generationJobAssetId(job);
      track(assetId);
      const completed: unknown = await completion;
      if (!isProject(completed)) {
        // Bridges without the native command fall back to the distributed Temporal start.
        shared.starting.delete(job.id);
        return await startTemporal(job);
      }
      await commitProject(completed);
      return true;
    } catch (error) {
      // The native runner persists the failed job before rejecting, so reload to show it.
      let latest = state().project;
      try {
        const failed = await loadSplitProjectFromFolder({ projectDir: state().projectDir });
        if (isProject(failed)) {
          await commitProject(failed);
          latest = failed;
        }
      } catch {
        // Keep the provider or setup error as the actionable message.
      }
      // An agent Undo removed the generation while it ran; there is nothing left to report.
      if (!hasGeneratedAsset(latest, generationJobAssetId(job))) return false;
      return block(errorMessage(error));
    } finally {
      shared.starting.delete(job.id);
    }
  }

  /**
   * Records every job and queued asset (plus extra actions) in one batch, then starts the workflows
   * one after another, each after the previous one settles.
   */
  async function queue(assets: readonly QueuedAsset[], extra: (assetId: string) => ProjectAction[] = () => []): Promise<GenerationStart[] | null> {
    const createdAt = now();
    state().setLastError(null);
    try {
      const jobs: ProjectJobSummary[] = [];
      for (const asset of assets) {
        jobs.push(
          await buildJob(asset.id, createdAt, {
            name: asset.name ?? null,
            targetFolderId: asset.targetFolderId ?? null,
            placementIntent: asset.placementIntent ?? "library",
            prompt: asset.prompt,
            model: asset.model,
            references: asset.references,
            settings: asset.settings,
          }),
        );
      }
      const recorded = await state().applyActions(
        assets.flatMap((asset, index): ProjectAction[] => {
          const job = jobs[index];
          if (!job) return [];
          return [{ type: "recordJob", job }, { type: "recordGeneratedAsset", asset: { ...asset, status: "queued", outputs: [], createdAt } }, ...extra(asset.id)];
        }),
      );
      if (!recorded) return null;
      return startInOrder(jobs, startWorkflow);
    } catch (error) {
      block(errorMessage(error));
      return null;
    }
  }

  /** Starts each job's workflow after the previous one settles. */
  function startInOrder(jobs: readonly ProjectJobSummary[], start: (job: ProjectJobSummary) => Promise<boolean>): GenerationStart[] {
    let previous: Promise<boolean> | null = null;
    return jobs.map((job) => {
      const run = () => start(job);
      const workflow: Promise<boolean> = previous ? previous.then(run, run) : run();
      previous = workflow;
      return { assetId: generationJobAssetId(job), workflow };
    });
  }

  async function queueOne(asset: QueuedAsset, extra?: (assetId: string) => ProjectAction[]): Promise<GenerationStart | null> {
    return (await queue([asset], extra))?.[0] ?? null;
  }

  function startGeneration(request: MediaGenerationRequest): Promise<GenerationStart | null> {
    return queueOne(
      {
        id: generatedMediaAssetId(),
        kind: request.kind,
        name: request.name,
        targetFolderId: request.targetFolderId,
        placementIntent: request.placementIntent,
        prompt: request.prompt,
        model: request.model,
        references: request.references,
        settings: request.settings,
        parentAssetId: null,
        retryOfAssetId: null,
      },
      (id) => {
        const placeholder = generatedTimelinePlaceholderAction(state().project, id, request);
        return placeholder ? [placeholder] : [];
      },
    );
  }

  async function startPlan(plan: RequestPlan): Promise<GenerationStart | null> {
    if ("blocked" in plan) {
      block(plan.blocked);
      return null;
    }
    return startGeneration(plan.request);
  }

  return {
    startGeneration,

    async rerun(assetId) {
      const asset = state().project.generatedAssets.find((candidate) => candidate.id === assetId);
      if (!asset || asset.prompt.trim().length === 0) {
        block(`Generated asset ${assetId} cannot be rerun without a prompt.`);
        return null;
      }
      return queueOne(variationOf(asset, generatedVariationId(asset.id), asset.name ?? null, asset.prompt));
    },

    async createVariations(assetId, count) {
      const asset = state().project.generatedAssets.find((candidate) => candidate.id === assetId);
      if (!asset) {
        block("That generation is no longer in the project.");
        return null;
      }
      const drafts = variationDraftsForAsset(asset, count);
      if (count <= 1) {
        const [draft] = drafts;
        if (draft) return queue([variationOf(asset, generatedVariationId(asset.id), asset.name ?? null, draft.prompt)]);
        block("This generation has no prompt to vary.");
        return null;
      }
      if (drafts.length < 2) {
        block("A variation set needs at least two prompts.");
        return null;
      }
      return queue(drafts.map((draft, index) => variationOf(asset, generatedVariationSetId(asset.id, index), draft.name, draft.prompt)));
    },

    async replaceWithOutput(itemId, mediaId) {
      const { project, projectDir } = state();
      const blocked = generatedOutputReplacementBlocker(project, projectDir, itemId, mediaId);
      if (blocked) return block(blocked);
      return (await state().applyActions([{ type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId, mediaId } }])) !== null;
    },

    upscale: (mediaId, context) => startPlan(upscaleRequestPlan(state().project, mediaId, context)),

    videoToAudio: (mediaId, kind, context) => startPlan(videoAudioRequestPlan(state().project, mediaId, kind, context)),

    startRecorded(jobIds) {
      const jobs = jobIds.flatMap((jobId) => queuedGeneration(state().project, jobId) ?? []);
      // An Undo between starts removes the later jobs, so each is looked up again when its turn comes.
      return startInOrder(jobs, async (job) => {
        const current = queuedGeneration(state().project, job.id);
        return current ? startWorkflow(current) : true;
      });
    },

    forgetRemoved(assetIds) {
      for (const assetId of assetIds) shared.awaitingOutput.delete(assetId);
    },

    async cancelGeneration(assetId) {
      const { project, projectDir } = state();
      if (projectDir.trim().length === 0) return false;
      const job =
        project.jobs.find((candidate) => candidate.id === assetId) ??
        project.jobs.find((candidate) => candidate.kind === "generate_media" && generationJobAssetId(candidate) === assetId);
      if (!job) return block(`Generated media workflow job is missing for ${assetId}.`);
      state().setLastError(null);
      try {
        if (isRealGeneration(job)) {
          const cancellation = await cancelGenerateMediaInProcess({ projectDir, jobId: job.id, updatedAt: now() });
          await commitProject(cancellation.project);
          const provider = job.providerRequest;
          if (provider?.cancelUrl && providerCancelProviders.has(provider.provider)) {
            try {
              await cancelGenerateMediaProviderRequestInSplitProjectFolder({ projectDir, jobId: job.id });
            } catch (error) {
              return block(`Generation was cancelled locally, but the provider cancellation request failed: ${errorMessage(error)}`);
            }
          }
          return true;
        }
        const actions = await buildTemporalGenerateMediaFailureActions({ job, assetId: generationJobAssetId(job), runId: job.workflow?.runId ?? null, updatedAt: now() });
        return (await state().applyActions(actions)) !== null;
      } catch (error) {
        return block(errorMessage(error));
      }
    },

    async retryDownload(assetId, outputMediaId) {
      const { project, projectDir } = state();
      const asset = project.generatedAssets.find((candidate) => candidate.id === assetId);
      if (!asset) return block(`Generated asset ${assetId} was not found.`);
      const requested = outputMediaId ? asset.outputs.find((output) => output.mediaId === outputMediaId) ?? null : null;
      if (outputMediaId && !requested) return block(`Generated asset ${assetId} has no output media ${outputMediaId}.`);
      const output =
        requested ??
        asset.outputs.find((candidate) => !project.media.some((media) => media.id === candidate.mediaId)) ??
        asset.outputs.find((candidate) => Boolean(candidate.sourceUrl?.trim()));
      if (!output) return block(`Generated asset ${assetId} has no retriable output download.`);
      if (!output.sourceUrl?.trim()) {
        return block(`Generated asset ${assetId} output ${output.mediaId} has no provider retry URL. Rerun generation to recreate the file.`);
      }
      if (!usesSplitFolder(project, projectDir)) {
        return block(`Generated asset ${assetId} output ${output.mediaId} can only be retried from a split project folder.`);
      }
      state().setLastError(null);
      try {
        const result = await retryGeneratedAssetOutputDownloadInSplitProjectFolder({ projectDir, assetId, outputMediaId: output.mediaId });
        await commitProject(result.project);
        return true;
      } catch (error) {
        return block(errorMessage(error));
      }
    },

    async completeMock(assetId) {
      const { projectDir } = state();
      if (projectDir.trim().length === 0) return false;
      state().setLastError(null);
      try {
        const result = await completeMockGeneratedAssetInSplitProjectFolder({ projectDir, assetId, updatedAt: now(), replacementItemId: null });
        await state().mergeLoadedProject(result.project);
        await placeOutput(assetId, true);
        return true;
      } catch (error) {
        return block(errorMessage(error));
      }
    },
  };
}

/**
 * Stops watching the store for generation output; `EditorRoot` calls it (after `stopPolling`) when the
 * editor unmounts. The next tracked start subscribes again, so a StrictMode remount keeps working.
 */
export function disposeGenerationRuntime(store: EditorStore): void {
  const runtime = runtimes.get(store);
  if (!runtime) return;
  runtime.unsubscribe?.();
  runtime.unsubscribe = null;
}

/** The generation service bound to the editor store; stable for the lifetime of the store. */
export function useGenerationService(): GenerationService {
  const store = useEditorStoreApi();
  return useMemo(() => createGenerationService(store), [store]);
}
