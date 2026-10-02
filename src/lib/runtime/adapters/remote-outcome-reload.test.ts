import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RemoteTransport } from "./remote-transport";
import { remoteProjectRevision } from "./remote-project-access";

const storageKey = "video-creater.remotePendingOutcomes.v1";
beforeEach(() => window.sessionStorage.clear());
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.useRealTimers(); });

function reply(init: RequestInit | undefined, result: unknown = null) {
  return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result }));
}
const lease = () => new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "private-lease", expiresAt: Date.now() / 1000 + 60 }));

it("records pending work before dispatch and restores its gate after reload without private payloads", async () => {
  let finish!: (response: Response) => void;
  let sent!: RequestInit;
  const first = new RemoteTransport({ csrfToken: "private-csrf", hostLabel: "Studio", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return lease();
    sent = init!;
    return new Promise<Response>((resolve) => { finish = resolve; });
  } });
  const pending = first.request("save_split_project_to_folder", { projectDir: "catalog-p", project: { id: "canonical-p", name: "PRIVATE PROJECT", media: [{ path: "/private/media.mp4" }] } });
  await vi.waitFor(() => expect(sent).toBeDefined());
  const raw = window.sessionStorage.getItem(storageKey);
  expect(raw).not.toBeNull();
  expect(raw).toContain(JSON.parse(String(sent.body)).requestId);
  for (const secret of ["private-csrf", "private-lease", "PRIVATE PROJECT", "/private/media.mp4", "payload"]) expect(raw).not.toContain(secret);
  const nextFetch = vi.fn(async (_url: RequestInfo | URL, init?: RequestInit) => reply(init));
  const restored = new RemoteTransport({ csrfToken: "new-csrf", hostLabel: "Studio", fetcher: nextFetch });
  await expect(restored.request("save_split_project_to_folder", { projectDir: "catalog-p" })).rejects.toMatchObject({ outcome: "unknown" });
  expect(nextFetch).not.toHaveBeenCalled();
  finish(reply(sent));
  await pending;
  expect(window.sessionStorage.getItem(storageKey)).toBe(raw);
  expect(restored.pendingOutcome("catalog-p")?.error.outcome).toBe("unknown");
});

it("restores unknown project creation across same-host reconnect while keeping other hosts writable", async () => {
  const first = new RemoteTransport({ csrfToken: "csrf", hostLabel: "Studio", fetcher: async () => { throw new Error("lost"); } });
  await expect(first.request("remote_create_project", { project: { name: "private" } })).rejects.toMatchObject({ outcome: "unknown" });
  const originalLocation = globalThis.location;
  vi.stubGlobal("location", { origin: "https://other-host.test" });
  const other = new RemoteTransport({ csrfToken: "csrf", hostLabel: "Other", fetcher: async (_url, init) => reply(init) });
  await expect(other.request("remote_create_project")).resolves.toBeNull();
  vi.stubGlobal("location", originalLocation);
  const same = new RemoteTransport({ csrfToken: "csrf", hostLabel: "Renamed Studio", fetcher: async (_url, init) => reply(init) });
  await expect(same.request("remote_create_project")).rejects.toMatchObject({ outcome: "unknown" });
  await expect(same.request("remote_list_projects")).resolves.toBeNull();
});

it.each([401, 403, 409])("clears known HTTP %s rejection before reload instead of restoring uncertainty", async (status) => {
  const first = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: false, error: { code: "conflict" } }), { status }) });
  await expect(first.request("remote_create_project")).rejects.toMatchObject({ outcome: "rejected" });
  const next = new RemoteTransport({ csrfToken: "new", fetcher: async (_url, init) => reply(init) });
  await expect(next.request("remote_create_project")).resolves.toBeNull();
});

it("rejects writes before sending when storage fails but still permits reads and cancellation", async () => {
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("quota"); });
  const operations: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => {
    operations.push(JSON.parse(String(init?.body)).operation);
    return reply(init);
  } });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  await transport.request("remote_list_projects");
  await transport.request("cancel_render_job_in_split_project_folder");
  expect(operations).toEqual(["remote_list_projects", "cancel_render_job_in_split_project_folder"]);
});

it("fails closed on corrupt recovery metadata without blocking safe reads", async () => {
  window.sessionStorage.setItem(storageKey, "{broken");
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => reply(init) });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  await expect(transport.request("remote_list_projects")).resolves.toBeNull();
  expect(window.sessionStorage.getItem(storageKey)).toBe("{broken");
});

it("requires the retained canonical identity when refreshing restored work", async () => {
  const first = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url) => String(url).endsWith("/lease") ? lease() : Promise.reject(new Error("lost")) });
  await expect(first.request("save_split_project_to_folder", { projectDir: "catalog-p", project: { id: "canonical-p" } })).rejects.toMatchObject({ outcome: "unknown" });
  let canonicalId = "replacement-p";
  const restored = new RemoteTransport({ csrfToken: "new", fetcher: async (_url, init) => reply(init, { id: canonicalId, contentRevision: 2 }) });
  await expect(restored.reconcileProject("catalog-p")).rejects.toThrow("different project");
  expect(restored.pendingOutcome("catalog-p")).toBeDefined();
  canonicalId = "canonical-p";
  await expect(restored.reconcileProject("catalog-p")).resolves.toMatchObject({ id: "canonical-p" });
  const reloaded = new RemoteTransport({ csrfToken: "new", fetcher: async (url, init) => String(url).endsWith("/lease") ? lease() : reply(init) });
  await expect(reloaded.request("save_split_project_to_folder", { projectDir: "catalog-p" })).resolves.toBeNull();
});

it("keeps unrelated unknown markers when a different project succeeds", async () => {
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return lease();
    if (JSON.parse(String(init?.body)).projectId === "unconfirmed-p") throw new Error("lost");
    return reply(init);
  } });
  await expect(transport.request("save_split_project_to_folder", { projectDir: "unconfirmed-p" })).rejects.toMatchObject({ outcome: "unknown" });
  await transport.request("save_split_project_to_folder", { projectDir: "confirmed-p" });
  const reloaded = new RemoteTransport({ csrfToken: "new", fetcher: async (url, init) => String(url).endsWith("/lease") ? lease() : reply(init) });
  await expect(reloaded.request("save_split_project_to_folder", { projectDir: "unconfirmed-p" })).rejects.toMatchObject({ outcome: "unknown" });
  await expect(reloaded.request("save_split_project_to_folder", { projectDir: "confirmed-p" })).resolves.toBeNull();
});

it("keeps restored uncertainty when authorization expires during canonical refresh", async () => {
  const first = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url) => String(url).endsWith("/lease") ? lease() : Promise.reject(new Error("lost")) });
  await expect(first.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ outcome: "unknown" });
  const saved = window.sessionStorage.getItem(storageKey);
  const restored = new RemoteTransport({ csrfToken: "expired", fetcher: async () => new Response("{}", { status: 401 }) });
  await expect(restored.reconcileProject("p", "canonical-p")).rejects.toMatchObject({ code: "unauthorized", outcome: "rejected" });
  expect(window.sessionStorage.getItem(storageKey)).toBe(saved);
  expect(restored.pendingOutcome("p")).toBeDefined();
});

it.each([null, "[]"])("treats missing or empty storage as clear recovery state (%s)", async (value) => {
  if (value !== null) window.sessionStorage.setItem(storageKey, value);
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => reply(init) });
  await expect(transport.request("remote_create_project")).resolves.toBeNull();
  expect(JSON.parse(window.sessionStorage.getItem(storageKey)!)).toEqual([]);
});

it("rejects writes without dispatch when recovery storage cannot be read", async () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("disabled"); });
  const fetcher = vi.fn(async (_url: RequestInfo | URL, init?: RequestInit) => reply(init));
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  expect(fetcher).not.toHaveBeenCalled();
  await expect(transport.request("remote_list_projects")).resolves.toBeNull();
});

it("does not evict unresolved work when the bounded recovery store is full", async () => {
  const raw = JSON.stringify(Array.from({ length: 32 }, (_, index) => ({ hostOrigin: globalThis.location.origin, requestId: `browser-${index}`, operation: "save_split_project_to_folder", projectId: `p-${index}`, phase: "rpc", outcome: "unknown" })));
  window.sessionStorage.setItem(storageKey, raw);
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => reply(init) });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  expect(window.sessionStorage.getItem(storageKey)).toBe(raw);
});

it("does not permit canonical refresh to clear an operation that is still running", async () => {
  let finish!: (response: Response) => void;
  let sent!: RequestInit;
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return lease();
    sent = init!;
    return new Promise<Response>((resolve) => { finish = resolve; });
  } });
  const work = transport.request("save_split_project_to_folder", { projectDir: "p" });
  await vi.waitFor(() => expect(sent).toBeDefined());
  await expect(transport.reconcileProject("p")).rejects.toThrow("still running");
  finish(reply(sent));
  await work;
  expect(JSON.parse(window.sessionStorage.getItem(storageKey)!)).toEqual([]);
});

it("requires a canonical identity from the opened project when a restored marker has none", async () => {
  const first = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url) => String(url).endsWith("/lease") ? lease() : Promise.reject(new Error("lost")) });
  await expect(first.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ outcome: "unknown" });
  const restored = new RemoteTransport({ csrfToken: "new", fetcher: async (_url, init) => reply(init, { id: "canonical-p", contentRevision: 2 }) });
  await expect(restored.reconcileProject("p")).rejects.toThrow("identity");
  expect(restored.pendingOutcome("p")).toBeDefined();
  await expect(restored.reconcileProject("p", "canonical-p")).resolves.toMatchObject({ id: "canonical-p" });
});

it("does not clear a different persisted request when canonical reconciliation finishes", async () => {
  const first = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url) => String(url).endsWith("/lease") ? lease() : Promise.reject(new Error("lost")) });
  await expect(first.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ outcome: "unknown" });
  let finish!: (response: Response) => void;
  let sent!: RequestInit;
  const restored = new RemoteTransport({ csrfToken: "new", fetcher: async (_url, init) => { sent = init!; return new Promise<Response>((resolve) => { finish = resolve; }); } });
  const refreshed = restored.reconcileProject("p", "canonical-p");
  const markers = JSON.parse(window.sessionStorage.getItem(storageKey)!);
  markers[0].requestId = "browser-newer-request";
  const newer = JSON.stringify(markers);
  window.sessionStorage.setItem(storageKey, newer);
  finish(reply(sent, { id: "canonical-p", contentRevision: 2 }));
  await expect(refreshed).rejects.toThrow("changed");
  expect(restored.pendingOutcome("p")).toBeDefined();
  expect(window.sessionStorage.getItem(storageKey)).toBe(newer);
  expect(remoteProjectRevision("p")).toBeUndefined();
});

it("leaves no recovery marker when editing access is rejected before dispatch", async () => {
  const first = new RemoteTransport({ csrfToken: "csrf", fetcher: async () => new Response("{}", { status: 401 }) });
  await expect(first.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ outcome: "not_sent", phase: "lease" });
  expect(window.sessionStorage.getItem(storageKey)).toBeNull();
  const reloaded = new RemoteTransport({ csrfToken: "new", fetcher: async (url, init) => String(url).endsWith("/lease") ? lease() : reply(init) });
  await expect(reloaded.request("save_split_project_to_folder", { projectDir: "p" })).resolves.toBeNull();
});

it.each([
  "{}", "[{}]",
  JSON.stringify([{ hostOrigin: "https://host.test", requestId: "browser-p", operation: "save_split_project_to_folder", projectId: "p", phase: "rpc", outcome: ["unknown"] }]),
  JSON.stringify([{ hostOrigin: "https://host.test", requestId: "browser-p", operation: "save_split_project_to_folder", projectId: "/private/path", phase: "rpc", outcome: "unknown" }]),
  JSON.stringify([{ hostOrigin: "https://host.test", requestId: "browser-p", operation: "save_split_project_to_folder", projectId: "p", phase: "rpc", outcome: "unknown", payload: { secret: "private" } }]),
])("blocks mutation dispatch for invalid marker shapes or unsafe metadata", async (raw) => {
  window.sessionStorage.setItem(storageKey, raw);
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => reply(init) });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  expect(window.sessionStorage.getItem(storageKey)).toBe(raw);
});

it("rejects a late canonical refresh when a new mutation completed after it started", async () => {
  let finish!: (response: Response) => void;
  let snapshotRequest!: RequestInit;
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return lease();
    if (JSON.parse(String(init?.body)).operation === "read_project_snapshot_from_split_project_folder") {
      snapshotRequest = init!;
      return new Promise<Response>((resolve) => { finish = resolve; });
    }
    return reply(init, { id: "canonical-p", contentRevision: 2 });
  } });
  const refresh = transport.reconcileProject("p", "canonical-p");
  await transport.request("save_split_project_to_folder", { projectDir: "p", project: { id: "canonical-p" } });
  finish(reply(snapshotRequest, { id: "canonical-p", contentRevision: 1 }));
  await expect(refresh).rejects.toThrow("changed");
  expect(remoteProjectRevision("p")).toBe(2);
});

it("bounds cached canonical identities without discarding unresolved recovery markers", async () => {
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return lease();
    const request = JSON.parse(String(init?.body));
    if (request.operation === "load_split_project_from_folder") return reply(init, { id: `canonical-${request.projectId}`, contentRevision: 1 });
    throw new Error("lost");
  } });
  for (let index = 0; index < 33; index += 1) await transport.request("load_split_project_from_folder", { projectDir: `p-${index}` });
  await expect(transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p-0", actions: [] })).rejects.toMatchObject({ outcome: "unknown" });
  await expect(transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p-32", actions: [] })).rejects.toMatchObject({ outcome: "unknown" });
  const markers = JSON.parse(window.sessionStorage.getItem(storageKey)!);
  expect(markers).toHaveLength(2);
  expect(markers.find((marker: { projectId: string }) => marker.projectId === "p-0").canonicalProjectId).toBeUndefined();
  expect(markers.find((marker: { projectId: string }) => marker.projectId === "p-32").canonicalProjectId).toBe("canonical-p-32");
  expect(transport.pendingOutcome("p-0")).toBeDefined();
});

it("permits historical custom canonical IDs while requiring an opened identity after reload", async () => {
  const customId = "Custom project / 2020: edit α";
  const first = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url) => String(url).endsWith("/lease") ? lease() : Promise.reject(new Error("lost")) });
  await expect(first.request("save_split_project_to_folder", { projectDir: "catalog-p", project: { id: customId } })).rejects.toMatchObject({ outcome: "unknown" });
  expect(window.sessionStorage.getItem(storageKey)).not.toContain(customId);
  const restored = new RemoteTransport({ csrfToken: "new", fetcher: async (_url, init) => reply(init, { id: customId, contentRevision: 2 }) });
  await expect(restored.reconcileProject("catalog-p")).rejects.toThrow("identity");
  await expect(restored.reconcileProject("catalog-p", "different custom ID")).rejects.toThrow("different project");
  await expect(restored.reconcileProject("catalog-p", customId)).resolves.toMatchObject({ id: customId });
});

it("accepts a loaded historical custom identity for an ordinary subsequent mutation", async () => {
  const customId = "Historic custom project / α";
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => String(url).endsWith("/lease") ? lease() : reply(init, { id: customId, contentRevision: 1 }) });
  await transport.request("load_split_project_from_folder", { projectDir: "catalog-p" });
  await expect(transport.request("apply_project_actions_to_split_project_folder", { projectDir: "catalog-p", actions: [] })).resolves.toMatchObject({ id: customId });
});
