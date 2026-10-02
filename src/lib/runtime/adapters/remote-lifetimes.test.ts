import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { BackendClient } from "../backend-client";
import { clearRemoteProjectAccessForTests } from "./remote-project-access";
import { RemoteTransport, type RemoteRequestTiming } from "./remote-transport";
import { remoteProjectAccess, remoteProjectRevision } from "./remote-project-access";
import { clearRemoteResourceUrlsForTests, remoteMediaReadiness } from "./remote-resource-cache";
import { remoteUnknownOutcome } from "./remote-outcome-state";
import { remoteOperationBypassesQueue, remoteOperationClass } from "./remote-operation-policy";

beforeEach(() => { window.sessionStorage.clear(); clearRemoteProjectAccessForTests(); clearRemoteResourceUrlsForTests(); });
afterEach(() => { vi.useRealTimers(); });

it.each(["conflict", "busy", "rate_limited"])("preserves typed %s outcomes and retry hints through BackendClient", async (code) => {
  let requestId = "";
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => {
    requestId = (JSON.parse(String(init?.body)) as { requestId: string }).requestId;
    return new Response(JSON.stringify({ requestId, ok: false, error: { code, message: "Try again later", retryAfterMs: 250 } }), { status: code === "rate_limited" ? 429 : 409 });
  } });
  const client = new BackendClient();
  client.install({ status: "connected", transport });
  await expect(client.request("get_platform_info")).rejects.toMatchObject({ code, requestId, retryAfterMs: 250, outcome: "rejected", phase: "rpc" });
});

it.each([[401, "unauthorized"], [403, "forbidden"]] as const)("treats HTTP %s authentication rejection as a known failure", async (status, code) => {
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async () => new Response(JSON.stringify({ message: "request authentication failed" }), { status }) });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ code, outcome: "rejected" });
});

it("treats an unrelated failure response as an unknown mutation outcome", async () => {
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url) => String(url).endsWith("/lease")
    ? new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }))
    : new Response(JSON.stringify({ requestId: "another-request", ok: false, error: { code: "conflict", message: "Unrelated rejection" } }), { status: 409 }) });
  await expect(transport.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ code: "outcome_unknown", outcome: "unknown" });
  expect(transport.pendingOutcome("p")).toBeDefined();
});

it("keeps render recovery under short mutation deadlines and the project write gate", () => {
  expect(remoteOperationClass("recover_render_attempt_in_split_project_folder", {})).toBe("mutation");
  expect(remoteOperationBypassesQueue("recover_render_attempt_in_split_project_folder")).toBe(false);
});

it("ignores retired-session RPC completions when installing media and editing access", async () => {
  let release!: (response: Response) => void;
  let requestId = "";
  const fetcher = vi.fn(async (_url: RequestInfo | URL, init?: RequestInit) => {
    requestId = (JSON.parse(String(init?.body)) as { requestId: string }).requestId;
    return new Promise<Response>((resolve) => { release = resolve; });
  });
  const retired = new RemoteTransport({ csrfToken: "old-csrf", fetcher });
  const pending = retired.request("load_split_project_from_folder", { projectDir: "p" });
  await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));
  new RemoteTransport({ csrfToken: "new-csrf", fetcher: async () => new Response("{}") });
  release(new Response(JSON.stringify({ requestId, ok: true, result: { contentRevision: 1, media: [{ relativePath: "old.mp4" }] } })));
  await pending;
  await Promise.resolve();
  expect(fetcher).toHaveBeenCalledTimes(1);
  expect(remoteMediaReadiness("p").status).toBe("idle");
  expect(remoteProjectAccess("p").mode).toBe("unknown");
});

it("ignores a retired-session editor lease completion", async () => {
  let release!: (response: Response) => void;
  let completed = false;
  const retired = new RemoteTransport({ csrfToken: "old-csrf", timingHandler: (span) => { if (span.operation === "acquire_editor_lease") completed = true; }, fetcher: async (url, init) => String(url).endsWith("/lease")
    ? new Promise<Response>((resolve) => { release = resolve; })
    : new Response(JSON.stringify({ requestId: (JSON.parse(String(init?.body)) as { requestId: string }).requestId, ok: true, result: { contentRevision: 1, media: [] } })) });
  await retired.request("load_split_project_from_folder", { projectDir: "p" });
  new RemoteTransport({ csrfToken: "new-csrf", fetcher: async () => new Response("{}") });
  release(new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "old-lease", expiresAt: Date.now() / 1000 + 30 })));
  await vi.waitFor(() => expect(completed).toBe(true));
  expect(remoteProjectAccess("p").mode).toBe("unknown");
});

it("does not clear a newer session's unknown edit when an old reconciliation completes", async () => {
  let release!: (response: Response) => void;
  let snapshotRequestId = "";
  const retired = new RemoteTransport({ csrfToken: "old-csrf", fetcher: async (_url, init) => {
    snapshotRequestId = (JSON.parse(String(init?.body)) as { requestId: string }).requestId;
    return new Promise<Response>((resolve) => { release = resolve; });
  } });
  const oldSnapshot = retired.reconcileProject("p").catch((error: unknown) => error);
  await vi.waitFor(() => expect(snapshotRequestId).not.toBe(""));
  const current = new RemoteTransport({ csrfToken: "new-csrf", fetcher: async (url) => String(url).endsWith("/lease")
    ? new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "new-lease", expiresAt: Date.now() / 1000 + 30 }))
    : new Response(JSON.stringify({ requestId: "unrelated", ok: true, result: {} })) });
  await expect(current.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ outcome: "unknown" });
  const currentUnknown = remoteUnknownOutcome("p");
  release(new Response(JSON.stringify({ requestId: snapshotRequestId, ok: true, result: { contentRevision: 1, media: [] } })));
  await expect(oldSnapshot).resolves.toBeInstanceOf(Error);
  expect(remoteUnknownOutcome("p")).toBe(currentUnknown);
});

it("retains and gates an unknown host creation while allowing catalog reads", async () => {
  let creates = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => {
    const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
    if (request.operation === "remote_create_project") { creates += 1; throw new Error("Response lost after creating project"); }
    return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: [{ projectId: "created" }] }));
  } });
  await expect(transport.request("remote_create_project", { project: { name: "First" } })).rejects.toMatchObject({ outcome: "unknown" });
  expect(transport.pendingOutcome()).toBeDefined();
  await expect(transport.request("remote_create_project", { project: { name: "Again" } })).rejects.toMatchObject({ outcome: "unknown" });
  await expect(transport.request("remote_list_projects")).resolves.toEqual([{ projectId: "created" }]);
  expect(creates).toBe(1);
});

it.each([
  { id: "different-project", contentRevision: 9 },
  { id: "canonical-p", contentRevision: -1 },
  { id: "canonical-p", contentRevision: 1.5 },
  { id: "canonical-p", contentRevision: Number.MAX_SAFE_INTEGER + 1 },
])("retains uncertainty when reconciliation returns an invalid canonical snapshot %j", async (snapshot) => {
  const operations: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
    operations.push(request.operation);
    if (request.operation === "save_split_project_to_folder") throw new Error("Response lost after commit");
    return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: { ...snapshot, media: [] } }));
  } });
  await expect(transport.request("save_split_project_to_folder", { projectDir: "catalog-p" })).rejects.toMatchObject({ outcome: "unknown" });
  const pending = transport.pendingOutcome("catalog-p");
  const notice = remoteUnknownOutcome("catalog-p");
  await expect(transport.reconcileProject("catalog-p", "canonical-p")).rejects.toBeInstanceOf(Error);
  expect(transport.pendingOutcome("catalog-p")).toBe(pending);
  expect(remoteUnknownOutcome("catalog-p")).toBe(notice);
  expect(remoteProjectRevision("catalog-p")).toBeUndefined();
  await expect(transport.request("save_split_project_to_folder", { projectDir: "catalog-p" })).rejects.toMatchObject({ outcome: "unknown" });
  expect(operations).toEqual(["save_split_project_to_folder", "read_project_snapshot_from_split_project_folder"]);
});

it("does not clear a newer unknown edit when an overlapping reconciliation finishes late", async () => {
  const snapshots: Array<() => void> = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
    if (request.operation === "save_split_project_to_folder") throw new Error("Response lost after commit");
    return new Promise<Response>((resolve) => {
      snapshots.push(() => resolve(new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: { id: "canonical-p", contentRevision: 1, media: [] } }))));
    });
  } });
  await expect(transport.request("save_split_project_to_folder", { projectDir: "catalog-p" })).rejects.toMatchObject({ outcome: "unknown" });
  const first = transport.reconcileProject("catalog-p", "canonical-p");
  const late = transport.reconcileProject("catalog-p", "canonical-p");
  await vi.waitFor(() => expect(snapshots).toHaveLength(2));
  snapshots[0]!();
  await first;
  await expect(transport.request("save_split_project_to_folder", { projectDir: "catalog-p" })).rejects.toMatchObject({ outcome: "unknown" });
  const newer = transport.pendingOutcome("catalog-p");
  const notice = remoteUnknownOutcome("catalog-p");
  snapshots[1]!();
  await expect(late).rejects.toBeInstanceOf(Error);
  expect(transport.pendingOutcome("catalog-p")).toBe(newer);
  expect(remoteUnknownOutcome("catalog-p")).toBe(notice);
});

it("bounds lost mutation responses, gates later writes and reconciles without replay", async () => {
  vi.useFakeTimers();
  const operations: string[] = [];
  const bodies: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", deadlines: { mutation: 10 }, fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
    operations.push(request.operation);
    bodies.push(String(init?.body));
    if (request.operation === "apply_project_actions_to_split_project_folder") return new Promise<Response>(() => undefined);
    return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: { contentRevision: 2, media: [] } }));
  } });
  let failure: unknown;
  const action = transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] }).catch((error: unknown) => { failure = error; });
  await vi.advanceTimersByTimeAsync(11);
  expect(failure).toMatchObject({ code: "outcome_unknown", outcome: "unknown", phase: "rpc" });
  await action;
  await expect(transport.request("save_split_project_to_folder", { projectDir: "p", project: {} })).rejects.toMatchObject({ code: "outcome_unknown" });
  await expect(transport.request("recover_render_attempt_in_split_project_folder", { projectDir: "p", jobId: "job" })).rejects.toMatchObject({ code: "outcome_unknown" });
  expect(operations).toEqual(["apply_project_actions_to_split_project_folder"]);
  const pending = transport.pendingOutcome("p");
  expect(pending?.envelope).toBe(bodies[0]);
  await expect(transport.reconcileProject("p")).resolves.toMatchObject({ contentRevision: 2 });
  await transport.request("save_split_project_to_folder", { projectDir: "p", project: {} });
  expect(operations).toEqual(["apply_project_actions_to_split_project_folder", "read_project_snapshot_from_split_project_folder", "save_split_project_to_folder"]);
  expect(JSON.parse(bodies.at(-1)!).expectedRevision).toBe(2);
});

it("reads progress and cancels while a same-project mutation is stalled", async () => {
  vi.useFakeTimers();
  const operations: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", deadlines: { mutation: 10 }, fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
    operations.push(request.operation);
    if (request.operation === "apply_project_actions_to_split_project_folder") return new Promise<Response>(() => undefined);
    return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: [] }));
  } });
  const action = transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] }).catch(() => undefined);
  await vi.advanceTimersByTimeAsync(0);
  void transport.request("load_job_progress_from_split_project_folder", { projectDir: "p", jobIds: [] });
  void transport.request("cancel_render_job_in_split_project_folder", { projectDir: "p" });
  await vi.advanceTimersByTimeAsync(0);
  expect(operations).toEqual(["apply_project_actions_to_split_project_folder", "load_job_progress_from_split_project_folder", "cancel_render_job_in_split_project_folder"]);
  await vi.advanceTimersByTimeAsync(11);
  await action;
});

it("expires queued work before dispatch and never executes that abandoned write", async () => {
  vi.useFakeTimers();
  let release!: (response: Response) => void;
  const operations: Array<{ requestId: string; operation: string }> = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", deadlines: { queue: 10, mutation: 100 }, fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
    operations.push(request);
    return new Promise<Response>((resolve) => { release = resolve; });
  } });
  const first = transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] });
  await vi.advanceTimersByTimeAsync(0);
  let failure: unknown;
  const second = transport.request("save_split_project_to_folder", { projectDir: "p", project: {} }).catch((error: unknown) => { failure = error; });
  await vi.advanceTimersByTimeAsync(11);
  expect(failure).toMatchObject({ code: "deadline_exceeded", outcome: "not_sent", phase: "queue" });
  release(new Response(JSON.stringify({ requestId: operations[0]!.requestId, ok: true, result: { project: { contentRevision: 2 } } })));
  await first;
  await second;
  expect(operations).toHaveLength(1);
});

it("reuses an unexpired editor lease and renews near expiry without retrying a rejected write", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(0);
  let leases = 0;
  let writes = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) {
      leases += 1;
      return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: `lease-${leases}`, expiresAt: Date.now() / 1000 + 30 }));
    }
    const request = JSON.parse(String(init?.body)) as { requestId: string };
    writes += 1;
    return new Response(JSON.stringify(writes === 3
      ? { requestId: request.requestId, ok: false, error: { code: "conflict", message: "lease changed" } }
      : { requestId: request.requestId, ok: true, result: { project: { contentRevision: writes } } }));
  } });
  await transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] });
  await transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] });
  expect(leases).toBe(1);
  vi.setSystemTime(26_000);
  await expect(transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] })).rejects.toMatchObject({ code: "conflict", outcome: "rejected" });
  expect(leases).toBe(2);
  expect(writes).toBe(3);
  await transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] });
  expect(leases).toBe(3);
});

it("bounds a stalled lease before the mutation is sent and permits a later fresh request", async () => {
  vi.useFakeTimers();
  let leases = 0;
  let rpcs = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", deadlines: { lease: 10 }, fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) {
      leases += 1;
      if (leases === 1) return new Promise<Response>(() => undefined);
      return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    }
    rpcs += 1;
    const request = JSON.parse(String(init?.body)) as { requestId: string };
    return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: null }));
  } });
  let failure: unknown;
  const first = transport.request("save_split_project_to_folder", { projectDir: "p", project: {} }).catch((error: unknown) => { failure = error; });
  await vi.advanceTimersByTimeAsync(11);
  await first;
  expect(failure).toMatchObject({ code: "deadline_exceeded", phase: "lease", outcome: "not_sent" });
  expect(rpcs).toBe(0);
  await transport.request("save_split_project_to_folder", { projectDir: "p", project: {} });
  expect(rpcs).toBe(1);
});

it("includes a stalled response body in the mutation deadline", async () => {
  vi.useFakeTimers();
  const transport = new RemoteTransport({ csrfToken: "csrf", deadlines: { mutation: 10 }, fetcher: async (url) => String(url).endsWith("/lease")
    ? new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }))
    : { json: () => new Promise(() => undefined) } as Response });
  let failure: unknown;
  const request = transport.request("save_split_project_to_folder", { projectDir: "p", project: {} }).catch((error: unknown) => { failure = error; });
  await vi.advanceTimersByTimeAsync(11);
  await request;
  expect(failure).toMatchObject({ code: "outcome_unknown", phase: "rpc" });
});

it("returns committed creation despite an unavailable post-create lease", async () => {
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => String(url).endsWith("/lease")
    ? new Response(JSON.stringify({ message: "Editing access temporarily unavailable" }), { status: 503 })
    : new Response(JSON.stringify({ requestId: (JSON.parse(String(init?.body)) as { requestId: string }).requestId, ok: true, result: { catalogProjectId: "created", project: { contentRevision: 1, media: [] } } })) });
  await expect(transport.request("remote_create_project")).resolves.toMatchObject({ catalogProjectId: "created" });
  await vi.waitFor(() => expect(remoteProjectAccess("created")).toMatchObject({ mode: "unavailable" }));
});

it("bounds stalled media independently while preserving the successful RPC", async () => {
  vi.useFakeTimers();
  const transport = new RemoteTransport({ csrfToken: "csrf", deadlines: { ticket: 10 }, fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    if (String(url).endsWith("/resource-tickets/media")) return new Promise<Response>(() => undefined);
    return new Response(JSON.stringify({ requestId: (JSON.parse(String(init?.body)) as { requestId: string }).requestId, ok: true, result: { project: { contentRevision: 2, media: [{ relativePath: "media/a.mp4" }] } } }));
  } });
  await expect(transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] })).resolves.toMatchObject({ project: { contentRevision: 2 } });
  await vi.advanceTimersByTimeAsync(11);
  expect(remoteMediaReadiness("p")).toMatchObject({ status: "failed", message: "Project media did not respond. Retry to reconnect." });
});

it("rejects excess queued writes while preserving admitted work and cancellation", async () => {
  vi.useFakeTimers();
  const transport = new RemoteTransport({ csrfToken: "csrf", deadlines: { mutation: 10 }, maximumPendingPerProject: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    const request = JSON.parse(String(init?.body)) as { operation: string; requestId: string };
    if (request.operation.startsWith("cancel_")) return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: true }));
    return new Promise<Response>(() => undefined);
  } });
  const pending = transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] }).catch(() => undefined);
  await expect(transport.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ code: "busy", phase: "queue", outcome: "not_sent" });
  await expect(transport.request("cancel_render_job_in_split_project_folder", { projectDir: "p" })).resolves.toBe(true);
  await vi.advanceTimersByTimeAsync(11);
  await pending;
});

it.each([20, 80, 150])("records request phases under controlled %i ms RTT and avoids live-ticket renewal", async (rtt) => {
  vi.useFakeTimers();
  vi.setSystemTime(0);
  const spans: RemoteRequestTiming[] = [];
  const calls: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", timingHandler: (span) => { spans.push(span); }, fetcher: async (url, init) => {
    calls.push(String(url));
    await new Promise((resolve) => setTimeout(resolve, rtt));
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    if (String(url).endsWith("/resource-tickets/media")) return new Response(JSON.stringify({ urls: { "p/media/a.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" } }));
    const request = JSON.parse(String(init?.body)) as { requestId: string };
    return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: { project: { contentRevision: 2, media: [{ relativePath: "media/a.mp4" }] } } }));
  } });
  let firstSettledAt = 0;
  const first = transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] }).then(() => { firstSettledAt = Date.now(); });
  await vi.advanceTimersByTimeAsync(3 * rtt);
  await first;
  expect(firstSettledAt).toBe(2 * rtt);
  const cachedStart = Date.now();
  let cachedSettledAt = 0;
  const second = transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] }).then(() => { cachedSettledAt = Date.now(); });
  await vi.advanceTimersByTimeAsync(rtt);
  await second;
  expect(cachedSettledAt - cachedStart).toBe(rtt);
  const requestSpans = spans.filter((span) => span.operation === "apply_project_actions_to_split_project_folder");
  expect(requestSpans.filter((span) => span.phase === "lease").map((span) => span.durationMs)).toEqual([rtt, 0]);
  expect(requestSpans.filter((span) => span.phase === "rpc").map((span) => span.durationMs)).toEqual([rtt, rtt]);
  expect(requestSpans.filter((span) => span.phase === "ticket").map((span) => span.durationMs)).toEqual([rtt]);
  expect(requestSpans.filter((span) => span.phase === "queue").map((span) => span.durationMs)).toEqual([0, 0]);
  expect(calls.filter((url) => url.endsWith("/lease"))).toHaveLength(1);
  expect(calls.filter((url) => url.endsWith("/resource-tickets/media"))).toHaveLength(1);
});
