import { describe, expect, it } from "vitest";
import { workflowNeverStartedReason, workflowStartFailureActions } from "@/lib/jobs/start-failure";
import type { ProjectJobSummary } from "@/lib/project";

function job(id: string, kind: string, input: Record<string, unknown> = {}): ProjectJobSummary {
  return {
    id,
    kind,
    status: "queued",
    updatedAt: "2026-09-16T10:00:00.000Z",
    startRequest: { workflowId: `wf/${id}`, workflowType: "Workflow", taskQueue: "q", input, searchAttributes: {}, activityTypes: [], idReusePolicy: "rejectDuplicate" },
  };
}

describe("workflowStartFailureActions", () => {
  it("fails the job with the never-started reason", () => {
    expect(workflowStartFailureActions(job("export-1", "export_media"), "2026-09-16T10:01:00.000Z")).toEqual([
      { type: "recordJobFailure", jobId: "export-1", reason: "The workflow never started.", updatedAt: "2026-09-16T10:01:00.000Z", runId: null },
    ]);
    expect(workflowNeverStartedReason).toBe("The workflow never started.");
  });

  it("also fails a generation's asset", () => {
    expect(workflowStartFailureActions(job("job-asset-1", "generate_media", { assetId: "asset-1" }), "2026-09-16T10:01:00.000Z")).toEqual([
      { type: "recordJobFailure", jobId: "job-asset-1", reason: "The workflow never started.", updatedAt: "2026-09-16T10:01:00.000Z", runId: null },
      { type: "updateGeneratedAssetStatus", assetId: "asset-1", status: "failed" },
    ]);
  });
});
