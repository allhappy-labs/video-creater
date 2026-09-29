import { generationJobAssetId } from "@/lib/jobs/activity-records";
import { workflowStartFailureActions } from "@/lib/jobs/start-failure";
import { buildFallbackTemporalStartResultAction, mockTemporalRunId } from "@/lib/jobs/temporal-fallback";
import {
  buildTemporalStartResultAction,
  startTemporalWorkflow,
  type ProjectAction,
  type ProjectJobSummary,
  type TemporalWorkflowStartResult,
  type VideoProject,
} from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { EditorStore } from "../store/editor-store";

function now(): string {
  return new Date().toISOString();
}

export function usesSplitFolder(project: VideoProject, projectDir: string): boolean {
  return project.schemaVersion >= 2 && projectDir.trim().length > 0;
}

export function isRealGeneration(job: ProjectJobSummary): boolean {
  return job.kind === "generate_media" && job.startRequest?.input.mockMode === false;
}

/** What the generation service shares with its workflow starts. */
interface GenerationWorkflowStartContext {
  readonly store: EditorStore;
  /** Jobs whose workflow start is in flight, so a double start never runs twice. */
  readonly starting: Set<string>;
  readonly block: (reason: string) => false;
  /** Awaits a started real generation's output. */
  readonly track: (assetId: string) => void;
  readonly errorMessage: (error: unknown) => string;
}

export interface GenerationWorkflowStarts {
  /** Pre-cut `startMockGenerateMediaWorkflow`. */
  startMockWorkflow(job: ProjectJobSummary): Promise<boolean>;
  /** Pre-cut `startQueuedTemporalWorkflow`; a start that fails fails the job and its asset. */
  startTemporal(job: ProjectJobSummary): Promise<boolean>;
}

export function createGenerationWorkflowStarts({ store, starting, block, track, errorMessage }: GenerationWorkflowStartContext): GenerationWorkflowStarts {
  const state = () => store.getState();

  /** The workflow never started: fail the job and asset so the task doesn't stay queued, then say why. */
  async function failStart(job: ProjectJobSummary, reason: string): Promise<false> {
    await state().applyActions(workflowStartFailureActions(job, now()));
    return block(reason);
  }

  return {
    async startMockWorkflow(job) {
      const { project, projectDir } = state();
      if (!job.startRequest || !usesSplitFolder(project, projectDir)) return true;
      const input = { job, runId: mockTemporalRunId(job.id), updatedAt: now() };
      let startAction: ProjectAction;
      try {
        startAction = await buildTemporalStartResultAction(input);
      } catch (error) {
        if (!isBackendUnavailableError(error)) throw error;
        startAction = buildFallbackTemporalStartResultAction(input);
      }
      return (await state().applyActions([startAction, { type: "updateGeneratedAssetStatus", assetId: generationJobAssetId(job), status: "running" }])) !== null;
    },

    async startTemporal(job) {
      if (!job.startRequest || starting.has(job.id)) return true;
      starting.add(job.id);
      try {
        let result: TemporalWorkflowStartResult;
        try {
          result = await startTemporalWorkflow({ job });
        } catch (error) {
          return await failStart(job, errorMessage(error));
        }
        if (result.status !== "started" || !result.runId) return await failStart(job, result.message);
        const action = await buildTemporalStartResultAction({ job, runId: result.runId, updatedAt: now() });
        const applied = await state().applyActions([action]);
        if (isRealGeneration(job)) track(generationJobAssetId(job));
        return applied !== null;
      } catch (error) {
        return block(errorMessage(error));
      } finally {
        starting.delete(job.id);
      }
    },
  };
}
