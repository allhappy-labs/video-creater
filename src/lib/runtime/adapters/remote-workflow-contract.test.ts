import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RemoteTransport } from "./remote-transport";
import { clearRemoteProjectAccessForTests } from "./remote-project-access";
import { clearRemoteResourceUrlsForTests } from "./remote-resource-cache";

beforeEach(() => { window.sessionStorage.clear(); clearRemoteProjectAccessForTests(); clearRemoteResourceUrlsForTests(); });
afterEach(() => vi.unstubAllGlobals());

function host(loseRecordResponse = false, canonicalProjectId = "canonical-project") {
  const envelopes: Record<string, unknown>[] = [];
  const fetcher = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
    const request = JSON.parse(String(init?.body));
    if (String(url).endsWith("/lease")) return Response.json({ mode: "editor", editorLeaseToken: "lease", expiresAt: Math.floor(Date.now() / 1000) + 30, editorDisplayName: "Browser" });
    envelopes.push(request);
    if (loseRecordResponse && request.operation === "apply_project_actions_to_split_project_folder") throw new Error("lost record-job response");
    const ok = !String(request.operation).startsWith("build_");
    const result = ["load_split_project_from_folder", "read_project_snapshot_from_split_project_folder"].includes(request.operation) ? { id: canonicalProjectId, contentRevision: 4, media: [] }
      : request.operation === "remote_build_temporal_job_summary" ? { id: request.payload.jobId, kind: request.payload.kind, status: "queued" }
        : { accepted: true };
    return Response.json({ requestId: request.requestId, ok, result: ok ? result : undefined, error: ok ? undefined : { code: "not_found", message: "Internal desktop builder is unavailable" } });
  });
  return { transport: new RemoteTransport({ csrfToken: "csrf", fetcher }), envelopes };
}

it("routes workflow builders through validated remote endpoints using the opened catalog identity", async () => {
  const { transport, envelopes } = host();
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-project" });
  await expect(transport.request("build_temporal_job_summary", { projectId: "canonical-project", kind: "transcribe_media", jobId: "speech-job", status: "queued", updatedAt: "2026-10-03T00:00:00Z" })).resolves.toMatchObject({ id: "speech-job", kind: "transcribe_media" });
  expect(envelopes.at(-1)).toMatchObject({ operation: "remote_build_temporal_job_summary", projectId: "catalog-project" });
});

it("retains the opened project identity across successive job-summary results", async () => {
  const { transport, envelopes } = host();
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-project" });
  for (const jobId of ["speech-job-1", "speech-job-2"]) {
    await transport.request("build_temporal_job_summary", { projectId: "canonical-project", kind: "transcribe_media", jobId });
    expect(envelopes.at(-1)).toMatchObject({ operation: "remote_build_temporal_job_summary", projectId: "catalog-project" });
  }
});

it("keeps the project identity in recovery markers after building and recording a job", async () => {
  const { transport } = host(true);
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-project" });
  const job = await transport.request("build_temporal_job_summary", { projectId: "canonical-project", kind: "transcribe_media", jobId: "speech-job" });
  await expect(transport.request("apply_project_actions_to_split_project_folder", { projectDir: "catalog-project", actions: [{ type: "recordJob", job }] })).rejects.toMatchObject({ outcome: "unknown" });
  expect(JSON.parse(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")!)).toEqual([
    expect.objectContaining({ projectId: "catalog-project", canonicalProjectId: "canonical-project" }),
  ]);
  await expect(transport.reconcileProject("catalog-project", "canonical-project")).resolves.toMatchObject({ id: "canonical-project", contentRevision: 4 });
  expect(transport.pendingOutcome("catalog-project")).toBeUndefined();
});

it("routes historical custom project identities without persisting them in recovery markers", async () => {
  const customId = "Historic custom project / α";
  const { transport, envelopes } = host(true, customId);
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-project" });
  const job = await transport.request("build_temporal_job_summary", { projectId: customId, kind: "transcribe_media", jobId: "speech-job" });
  expect(envelopes.at(-1)).toMatchObject({ operation: "remote_build_temporal_job_summary", projectId: "catalog-project" });
  await expect(transport.request("apply_project_actions_to_split_project_folder", { projectDir: "catalog-project", actions: [{ type: "recordJob", job }] })).rejects.toMatchObject({ outcome: "unknown" });
  expect(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")).not.toContain(customId);
});

it("uses the nested workflow project locator for lease, revision and generation admission", async () => {
  const { transport, envelopes } = host();
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-project" });
  await transport.request("run_generate_media_in_process", { startRequest: { input: { projectId: "canonical-project", projectDir: "catalog-project", jobId: "generate-job" } }, updatedAt: "2026-10-03T00:00:00Z" });
  expect(envelopes.at(-1)).toMatchObject({ operation: "run_generate_media_in_process", projectId: "catalog-project", expectedRevision: 4, editorLeaseToken: "lease" });
});

it("routes recorded Temporal starts using the job's project locator", async () => {
  const { transport, envelopes } = host();
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-project" });
  await transport.request("start_temporal_workflow", { job: { id: "speech-job", startRequest: { input: { projectDir: "catalog-project" } } } });
  expect(envelopes.at(-1)).toMatchObject({ operation: "remote_start_temporal_workflow", projectId: "catalog-project", expectedRevision: 4, editorLeaseToken: "lease" });
});
