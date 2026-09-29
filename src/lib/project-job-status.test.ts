import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type ProjectJobSummary, type VideoProject } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";

// Mirrors src-tauri/tests/project_action/job_status.rs.
describe("local job status updates for a finished workflow run", () => {
  function projectWithJob(status: ProjectJobSummary["status"], extra: Partial<ProjectJobSummary> = {}): VideoProject {
    const job: ProjectJobSummary = {
      id: "export-1",
      kind: "export_media",
      status,
      updatedAt: "2026-09-17T10:05:00Z",
      workflow: { workflowId: "video-creater/project-1/export-media/export-1", workflowType: "VideoCreaterExportMediaWorkflow", taskQueue: "video-creater-workflows", runId: "run-1", activityTypes: [] },
      ...extra,
    };
    return { ...fixtureProject(), jobs: [job] };
  }

  it("keeps a finished job when its own run's late 'started' write arrives", () => {
    for (const [status, extra] of [
      ["completed", {}],
      ["failed", { failureReason: "The workflow failed." }],
      ["cancelled", {}],
    ] as const) {
      const finished = projectWithJob(status, extra);

      const next = applyProjectActionLocally(finished, { type: "updateJobStatus", jobId: "export-1", status: "running", updatedAt: "2026-09-17T10:06:00Z", runId: "run-1" });

      expect(next.jobs[0], status).toEqual(finished.jobs[0]);
    }
  });

  it("still lets a new run restart a finished job and a finished run record its result", () => {
    const restarted = applyProjectActionLocally(projectWithJob("failed"), { type: "updateJobStatus", jobId: "export-1", status: "running", updatedAt: "2026-09-17T10:06:00Z", runId: "run-2" });
    expect(restarted.jobs[0]).toMatchObject({ status: "running", workflow: { runId: "run-2" } });

    const completed = applyProjectActionLocally(projectWithJob("running"), { type: "updateJobStatus", jobId: "export-1", status: "completed", updatedAt: "2026-09-17T10:06:00Z", runId: "run-1" });
    expect(completed.jobs[0]).toMatchObject({ status: "completed", updatedAt: "2026-09-17T10:06:00Z" });
  });
});
