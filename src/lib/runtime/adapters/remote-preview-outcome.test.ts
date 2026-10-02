import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RemoteTransport } from "./remote-transport";
import type { RemoteDeadlines } from "./remote-operation-policy";

const storageKey = "video-creater.remotePendingOutcomes.v1";
const previews = ["capture_canonical_preview_frame_in_split_project_folder", "prepare_project_preview"];
beforeEach(() => window.sessionStorage.clear());
afterEach(() => vi.restoreAllMocks());

function fixture(rpc: (operation: string, init: RequestInit) => Promise<Response>, deadlines?: Partial<RemoteDeadlines>) {
  const acknowledgements: string[] = [];
  const fetcher = async (url: RequestInfo | URL, init?: RequestInit) => {
    const path = String(url);
    if (path.endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "fixture-lease", expiresAt: Date.now() / 1000 + 60 }));
    if (path === "/api/v1/request-outcomes") return new Response(JSON.stringify({ outcomes: [], nextCursor: null }));
    if (path.endsWith("/ack")) { acknowledgements.push(path); return new Response(JSON.stringify({ acknowledged: true })); }
    if (path === "/api/v1/resource-tickets/media") return new Response(JSON.stringify({ urls: {} }));
    const operation = (JSON.parse(String(init?.body)) as { operation: string }).operation;
    return rpc(operation, init!);
  };
  return { transport: new RemoteTransport({ csrfToken: "fixture-csrf", outcomeProtocol: 1, fetcher, ...(deadlines ? { deadlines } : {}) }), acknowledgements };
}
function success(init: RequestInit, result: unknown = null) {
  return new Response(JSON.stringify({ requestId: JSON.parse(String(init.body)).requestId, ok: true, result }));
}

it.each(previews)("does not persist or acknowledge a mutation receipt for in-flight read %s", async (operation) => {
  let sent!: RequestInit;
  let finish!: (response: Response) => void;
  const { transport, acknowledgements } = fixture(async (_operation, init) => {
    sent = init;
    return new Promise<Response>((resolve) => { finish = resolve; });
  });
  const pending = transport.request(operation, { projectDir: "catalog-p" });
  await vi.waitFor(() => expect(sent).toBeDefined());
  const retainedDuringRead = window.sessionStorage.getItem(storageKey);
  finish(success(sent));
  await pending;
  expect(retainedDuringRead).toBeNull();
  expect(acknowledgements).toEqual([]);
});

it.each(previews)("lost read response for %s stays unavailable and leaves the next edit writable", async (operation) => {
  const operations: string[] = [];
  const { transport } = fixture(async (name, init) => {
    operations.push(name);
    if (name === operation) throw new Error("lost preview response");
    return success(init, { project: { id: "canonical-p", contentRevision: 2 } });
  });
  await expect(transport.request(operation, { projectDir: "catalog-p" })).rejects.toMatchObject({ outcome: "unavailable" });
  expect(transport.pendingOutcome("catalog-p")).toBeUndefined();
  expect(window.sessionStorage.getItem(storageKey)).toBeNull();
  await expect(transport.request("save_split_project_to_folder", { projectDir: "catalog-p", project: { id: "canonical-p", contentRevision: 1 } })).resolves.toMatchObject({ project: { contentRevision: 2 } });
  expect(operations).toEqual([operation, "save_split_project_to_folder"]);
});

it.each(previews)("preserves the native job deadline for semantic read %s", async (operation) => {
  const { transport } = fixture(async (_name, init) => {
    await new Promise((resolve) => setTimeout(resolve, 20));
    return success(init);
  }, { read: 5, job: 100 });
  await expect(transport.request(operation, { projectDir: "catalog-p" })).resolves.toBeNull();
});

it.each([...previews, "preview_storage_cleanup", "mcp_client_configuration"])("migrates historical false read marker %s while preserving accepted edit guards", async (operation) => {
  const accepted = { hostOrigin: globalThis.location.origin, requestId: "browser-real-edit", operation: "save_split_project_to_folder", projectId: "other-p", phase: "rpc", outcome: "unknown" };
  window.sessionStorage.setItem(storageKey, JSON.stringify([
    { ...accepted, requestId: "browser-false-read", operation, projectId: previews.includes(operation) ? "catalog-p" : undefined },
    accepted,
  ]));
  const { transport } = fixture(async (_name, init) => success(init));
  expect(JSON.parse(window.sessionStorage.getItem(storageKey)!)).toEqual([accepted]);
  expect(transport.pendingOutcome("other-p")).toBeDefined();
  await expect(transport.request("save_split_project_to_folder", { projectDir: "catalog-p" })).resolves.toBeNull();
  await expect(transport.request("save_split_project_to_folder", { projectDir: "other-p" })).rejects.toMatchObject({ outcome: "unknown" });
});
