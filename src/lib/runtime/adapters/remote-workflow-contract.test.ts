import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RemoteTransport } from "./remote-transport";
import { clearRemoteProjectAccessForTests } from "./remote-project-access";
import { clearRemoteResourceUrlsForTests } from "./remote-resource-cache";

beforeEach(() => { window.sessionStorage.clear(); clearRemoteProjectAccessForTests(); clearRemoteResourceUrlsForTests(); });
afterEach(() => vi.unstubAllGlobals());

function host() {
  const envelopes: Record<string, unknown>[] = [];
  const fetcher = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
    const request = JSON.parse(String(init?.body));
    if (String(url).endsWith("/lease")) return Response.json({ mode: "editor", editorLeaseToken: "lease", expiresAt: Math.floor(Date.now() / 1000) + 30, editorDisplayName: "Browser" });
    envelopes.push(request);
    const ok = !String(request.operation).startsWith("build_");
    const result = request.operation === "load_split_project_from_folder" ? { id: "canonical-project", contentRevision: 4, media: [] } : { accepted: true };
    return Response.json({ requestId: request.requestId, ok, result: ok ? result : undefined, error: ok ? undefined : { code: "not_found", message: "Internal desktop builder is unavailable" } });
  });
  return { transport: new RemoteTransport({ csrfToken: "csrf", fetcher }), envelopes };
}

it("routes workflow builders through validated remote endpoints using the opened catalog identity", async () => {
  const { transport, envelopes } = host();
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-project" });
  await expect(transport.request("build_temporal_job_summary", { projectId: "canonical-project", kind: "transcribe_media", jobId: "speech-job", status: "queued", updatedAt: "2026-10-03T00:00:00Z" })).resolves.toEqual({ accepted: true });
  expect(envelopes.at(-1)).toMatchObject({ operation: "remote_build_temporal_job_summary", projectId: "catalog-project" });
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
