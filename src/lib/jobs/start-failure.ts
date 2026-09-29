import { generationJobAssetId } from "@/lib/jobs/activity-records";
import type { ProjectAction, ProjectJobSummary } from "@/lib/project";

/** The failure reason of a job whose workflow start failed or never recorded a run. */
export const workflowNeverStartedReason = "The workflow never started.";

/**
 * Bookkeeping for a failed workflow start: fails the job with the never-started reason, and a
 * generation's asset too, so the task doesn't stay queued forever.
 */
export function workflowStartFailureActions(job: ProjectJobSummary, updatedAt: string): ProjectAction[] {
  const actions: ProjectAction[] = [{ type: "recordJobFailure", jobId: job.id, reason: workflowNeverStartedReason, updatedAt, runId: null }];
  if (job.kind === "generate_media") actions.push({ type: "updateGeneratedAssetStatus", assetId: generationJobAssetId(job), status: "failed" });
  return actions;
}
