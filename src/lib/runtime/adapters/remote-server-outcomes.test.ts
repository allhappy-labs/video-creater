import { beforeEach, expect, it, vi } from "vitest";
import { parseServerRequestOutcome, ServerOutcomeClient } from "./remote-server-outcomes";
import { RemoteTransport } from "./remote-transport";
import { persistRemoteOutcomeMarker } from "./remote-outcome-storage";

const oldId = "browser-old-creation";
const entry = { requestId: oldId, operation: "remote_create_project", status: "pending", acknowledged: false, issuedAt: 100 };
const reply = (init?: RequestInit) => new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: null }));

beforeEach(() => { window.sessionStorage.clear(); });

it("restores a pending server creation after local recovery storage was cleared", async () => {
  let creations = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [entry] }));
    if (String(url).endsWith("/rpc")) creations += 1;
    return reply(init);
  } });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "unknown", requestId: oldId });
  expect(creations).toBe(0);
  expect(transport.pendingOutcome()?.marker.requestId).toBe(oldId);
});

it("accepts the real server's explicit null cursor and issues timestamped UUID request identities", async () => {
  let requestId = "";
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [], nextCursor: null }));
    if (String(url).endsWith("/ack")) return new Response(JSON.stringify({ acknowledged: true }));
    expect(init?.headers).toMatchObject({ "x-video-creater-outcome-protocol": "1" });
    requestId = JSON.parse(String(init?.body)).requestId;
    return reply(init);
  } });
  await expect(transport.request("remote_create_project")).resolves.toBeNull();
  expect(requestId).toMatch(/^browser-v2-\d+-[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
});

it("resolves a retained creation guard only from its durable terminal outcome", async () => {
  persistRemoteOutcomeMarker({ hostOrigin: window.location.origin, requestId: oldId, operation: "remote_create_project", phase: "rpc", outcome: "unknown" });
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [{ ...entry, status: "succeeded", catalogProjectId: "a".repeat(64), canonicalProjectId: "canonical-created" }] }));
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify({ acknowledged: true }), { status: 200 });
    return reply(init);
  } });
  await transport.request("remote_list_projects");
  expect(transport.pendingOutcome()).toBeUndefined();
  await expect(transport.request("remote_create_project")).resolves.toBeNull();
});

it("retains typed durable unknown outcomes instead of clearing them as a rejected write", async () => {
  const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (_url, init) => new Response(JSON.stringify({
    requestId: JSON.parse(String(init?.body)).requestId, ok: false,
    error: { code: "outcome_unknown", message: "The original request may have completed. Refresh canonical state." },
  })) });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ code: "outcome_unknown", outcome: "unknown" });
  expect(transport.pendingOutcome()?.marker.operation).toBe("remote_create_project");
});

it("keeps a running project outcome paused even when canonical reads are available", async () => {
  let snapshots = 0;
  let acknowledgements = 0;
  const receipt = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p" };
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt] }));
    if (String(url).endsWith("/ack")) { acknowledgements += 1; return new Response(JSON.stringify({ acknowledged: true })); }
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    snapshots += 1;
    return reply(init);
  } });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toMatchObject({ outcome: "unknown" });
  expect(snapshots).toBe(0);
  expect(acknowledgements).toBe(0);
  expect(transport.pendingOutcome("p")).toBeDefined();
});

it("uses a terminal receipt to authorize a separate canonical read and explicit acknowledgement", async () => {
  const receipt = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", status: "succeeded", contentRevision: 4 };
  let ackHeaders: HeadersInit | undefined;
  const transport = new RemoteTransport({ csrfToken: "private-csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt] }));
    if (String(url).endsWith("/ack")) { ackHeaders = init?.headers; return new Response(JSON.stringify({ acknowledged: true })); }
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    const result = JSON.parse(String(init?.body)).operation === "read_project_snapshot_from_split_project_folder" ? { id: "canonical-p", contentRevision: 5, name: "Current canonical project" } : null;
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result }));
  } });
  await transport.request("remote_list_projects");
  expect(transport.pendingOutcome("p")).toBeDefined();
  await expect(transport.reconcileProject("p", "canonical-p")).resolves.toEqual({ id: "canonical-p", contentRevision: 5, name: "Current canonical project" });
  expect(transport.pendingOutcome("p")).toBeUndefined();
  expect(ackHeaders).toMatchObject({ "x-csrf-token": "private-csrf" });
});

it("retains an interrupted project guard when explicit acknowledgement is rejected", async () => {
  const receipt = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", status: "interrupted" };
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt] }));
    if (String(url).endsWith("/ack")) return new Response("{}", { status: 409 });
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: "canonical-p", contentRevision: 5 } }));
  } });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toThrow("recovery");
  expect(transport.pendingOutcome("p")).toBeDefined();
  await expect(transport.request("save_split_project_to_folder", { projectDir: "p" })).rejects.toMatchObject({ outcome: "unknown" });
});

it.each([new Response("{}", { status: 503 }), new Response(JSON.stringify({ outcomes: [{ ...entry, bearerToken: "private-token" }] }))])("blocks writes when host recovery is unavailable or unsafe, while reads and cancellation remain usable", async (failure) => {
  const calls: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/ack")) return new Response(JSON.stringify({ acknowledged: true }));
    if (String(url).endsWith("/request-outcomes")) return failure.clone();
    calls.push(JSON.parse(String(init?.body)).operation);
    return reply(init);
  } });
  await expect(transport.request("remote_list_projects")).resolves.toBeNull();
  await expect(transport.request("cancel_render_job_in_split_project_folder", { projectDir: "p" })).resolves.toBeNull();
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  expect(calls).toEqual(["remote_list_projects", "cancel_render_job_in_split_project_folder"]);
  expect(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1") ?? "").not.toContain("private-token");
});

it("finds unresolved work after a full page of terminal creation receipts", async () => {
  const terminal = Array.from({ length: 256 }, (_, index) => ({ ...entry, requestId: `browser-terminal-${index}`, status: "rejected" }));
  const pages: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).includes("/ack")) return new Response(JSON.stringify({ acknowledged: true }));
    if (String(url).includes("/request-outcomes")) {
      pages.push(String(url));
      return new Response(JSON.stringify(String(url).includes("?after=") ? { outcomes: [entry] } : { outcomes: terminal, nextCursor: "browser-terminal-255" }));
    }
    return reply(init);
  } });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "unknown", requestId: oldId });
  expect(pages).toEqual(["/api/v1/request-outcomes", "/api/v1/request-outcomes?after=browser-terminal-255"]);
});

it("does not evict any unresolved receipt to fit the client recovery limit", async () => {
  const receipts = Array.from({ length: 33 }, (_, index) => ({ ...entry, requestId: `browser-pending-${index}`, operation: "save_split_project_to_folder", projectId: `p-${index}` }));
  let writes = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: receipts }));
    writes += 1; return reply(init);
  } });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  expect(writes).toBe(0);
});

it("bounds simultaneous best-effort acknowledgements", async () => {
  let active = 0;
  let peak = 0;
  const releases: (() => void)[] = [];
  const client = new ServerOutcomeClient(async () => {
    active += 1; peak = Math.max(peak, active);
    await new Promise<void>((resolve) => releases.push(resolve));
    active -= 1;
    return new Response(JSON.stringify({ acknowledged: true }));
  }, () => "csrf", () => true, 1000);
  for (let index = 0; index < 8; index += 1) client.bestEffortAcknowledge(`browser-terminal-${index}`);
  await Promise.resolve();
  const observedPeak = peak;
  for (let index = 0; index < 16; index += 1) { releases.splice(0).forEach((release) => release()); await new Promise((resolve) => setTimeout(resolve, 0)); }
  expect(observedPeak).toBe(1);
});

it.each(["outcome_unknown", "outcome_expired"])("retains %s as typed uncertainty without acknowledging it", async (code) => {
  let acknowledgements = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [], nextCursor: null }));
    if (String(url).endsWith("/ack")) { acknowledgements += 1; return new Response(JSON.stringify({ acknowledged: true })); }
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: false, error: { code, message: "The original outcome is retained." } }));
  } });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ code, outcome: "unknown" });
  expect(transport.pendingOutcome()).toBeDefined();
  expect(acknowledgements).toBe(0);
});

it("does not let a retired connection's delayed server list install new guards", async () => {
  let finish!: (response: Response) => void;
  let oldWrites = 0;
  const old = new RemoteTransport({ csrfToken: "old", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Promise<Response>((resolve) => { finish = resolve; });
    oldWrites += 1; return reply(init);
  } });
  const writing = old.request("remote_create_project");
  const current = new RemoteTransport({ csrfToken: "new", fetcher: async (_url, init) => reply(init) });
  finish(new Response(JSON.stringify({ outcomes: [entry] })));
  await expect(writing).rejects.toMatchObject({ outcome: "not_sent" });
  expect(current.pendingOutcome()).toBeUndefined();
  await expect(current.request("remote_create_project")).resolves.toBeNull();
  expect(oldWrites).toBe(0);
});

it("does not clear a newer project guard from an older terminal receipt", async () => {
  const newerId = "browser-newer-write";
  persistRemoteOutcomeMarker({ hostOrigin: window.location.origin, requestId: newerId, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", phase: "rpc", outcome: "unknown" });
  const older = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", status: "succeeded", contentRevision: 2 };
  let lookedUp = "";
  let acknowledgements = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [older] }));
    if (String(url).endsWith("/ack")) { acknowledgements += 1; return new Response(JSON.stringify({ acknowledged: true })); }
    lookedUp = String(url);
    return new Response(JSON.stringify({ ...older, requestId: newerId, status: "pending" }));
  } });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toMatchObject({ requestId: newerId, outcome: "unknown" });
  expect(lookedUp).toBe(`/api/v1/request-outcomes/${newerId}`);
  expect(transport.pendingOutcome("p")?.marker.requestId).toBe(newerId);
  expect(acknowledgements).toBe(0);
});

it("keeps the current connection's guard when an old acknowledgement completes late", async () => {
  const receipt = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", status: "interrupted" };
  let finishAck!: (response: Response) => void;
  let announceAck!: () => void;
  const ackStarted = new Promise<void>((resolve) => { announceAck = resolve; });
  const old = new RemoteTransport({ csrfToken: "old", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt] }));
    if (String(url).endsWith("/ack")) { announceAck(); return new Promise<Response>((resolve) => { finishAck = resolve; }); }
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: "canonical-p", contentRevision: 5 } }));
  } });
  const refresh = old.reconcileProject("p", "canonical-p");
  await ackStarted;
  window.sessionStorage.setItem("video-creater.remotePendingOutcomes.v1", JSON.stringify([{ hostOrigin: window.location.origin, requestId: "browser-current-write", operation: receipt.operation, projectId: "p", canonicalProjectId: "canonical-p", phase: "rpc", outcome: "unknown" }]));
  const current = new RemoteTransport({ csrfToken: "new", fetcher: async (_url, init) => reply(init) });
  finishAck(new Response(JSON.stringify({ acknowledged: true })));
  await expect(refresh).rejects.toThrow("connection changed");
  expect(current.pendingOutcome("p")?.marker.requestId).toBe("browser-current-write");
});

it("preserves known commit success when its best-effort server acknowledgement fails", async () => {
  let acknowledgements = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [] }));
    if (String(url).endsWith("/ack")) { acknowledgements += 1; return new Response("{}", { status: 503 }); }
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: 9999999999 }));
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: "canonical-p", contentRevision: 2 } }));
  } });
  await expect(transport.request("save_split_project_to_folder", { projectDir: "p" })).resolves.toEqual({ id: "canonical-p", contentRevision: 2 });
  expect(transport.pendingOutcome("p")).toBeUndefined();
  expect(acknowledgements).toBe(1);
});

it("lets an oversized receipt backlog be drained through explicit canonical reconciliation", async () => {
  const receipts = Array.from({ length: 33 }, (_, index) => ({ ...entry, requestId: `browser-pending-${index}`, operation: "save_split_project_to_folder", projectId: `p-${index}`, canonicalProjectId: `canonical-${index}`, status: "interrupted" }));
  let acknowledgements = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: receipts, nextCursor: null }));
    if (String(url).endsWith("/ack")) { acknowledgements += 1; return new Response(JSON.stringify({ acknowledged: true })); }
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipts[0]));
    const operation = JSON.parse(String(init?.body)).operation;
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: operation === "read_project_snapshot_from_split_project_folder" ? { id: "canonical-0", contentRevision: 5 } : null }));
  } });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  await expect(transport.reconcileProject("p-0", "canonical-0")).resolves.toEqual({ id: "canonical-0", contentRevision: 5 });
  expect(transport.pendingOutcome("p-0")).toBeUndefined();
  expect(acknowledgements).toBe(1);
});

it("keeps historical canonical identities out of browser recovery metadata", async () => {
  const historicalId = "Historic custom project / α";
  const receipt = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: historicalId, status: "interrupted" };
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt], nextCursor: null }));
    if (String(url).endsWith("/ack")) return new Response(JSON.stringify({ acknowledged: true }));
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: historicalId, contentRevision: 5 } }));
  } });
  await transport.request("remote_list_projects");
  expect(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")).not.toContain(historicalId);
  await expect(transport.reconcileProject("p", "another opened identity")).rejects.toThrow("different project");
  await expect(transport.reconcileProject("p", historicalId)).resolves.toMatchObject({ id: historicalId });
});


it("reads a full legitimate host journal including the reserved cancellation receipts", async () => {
  const total = 16384 + 1024;
  let calls = 0;
  const client = new ServerOutcomeClient(async () => {
    const first = calls++ * 256;
    const outcomes = Array.from({ length: Math.min(256, total - first) }, (_, index) => ({ ...entry, requestId: `browser-terminal-${String(first + index).padStart(5, "0")}`, status: "rejected" }));
    return new Response(JSON.stringify({ outcomes, nextCursor: outcomes.length ? outcomes[outcomes.length - 1]!.requestId : null }));
  }, () => "csrf", () => true, 1000);
  expect(await client.list()).toHaveLength(total);
  expect(calls).toBe(69);
});

it("acknowledges historical request identities without storing them as browser metadata", async () => {
  const requestId = "rpc:1 / α";
  const urls: string[] = [];
  const client = new ServerOutcomeClient(async (url) => {
    urls.push(String(url));
    return new Response(JSON.stringify(String(url).endsWith("/ack") ? { acknowledged: true } : { ...entry, requestId, status: "rejected" }));
  }, () => "csrf", () => true, 1000);
  expect((await client.get(requestId)).requestId).toBe(requestId);
  await client.acknowledge(requestId);
  expect(urls).toEqual([`/api/v1/request-outcomes/${encodeURIComponent(requestId)}`, `/api/v1/request-outcomes/${encodeURIComponent(requestId)}/ack`]);
  expect(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")).toBeNull();
});

it("blocks all writes for a nonpersistable historical guard but allows its explicit project recovery", async () => {
  const requestId = "rpc:1 / α";
  const receipt = { ...entry, requestId, operation: "save_split_project_to_folder", projectId: "p", status: "interrupted" };
  const calls: string[] = [];
  let acknowledgements = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt], nextCursor: null }));
    if (String(url).endsWith("/ack")) { acknowledgements += 1; return new Response(JSON.stringify({ acknowledged: true })); }
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    calls.push(JSON.parse(String(init?.body)).operation);
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: "canonical-p", contentRevision: 5 } }));
  } });
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "not_sent" });
  expect(transport.pendingOutcome("p")?.marker.requestId).toBe(requestId);
  expect(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1") ?? "").not.toContain(requestId);
  await expect(transport.request("cancel_render_job_in_split_project_folder", { projectDir: "p" })).resolves.toMatchObject({ id: "canonical-p" });
  await expect(transport.reconcileProject("p")).rejects.toThrow("Open the project");
  await expect(transport.reconcileProject("p", "canonical-p")).resolves.toMatchObject({ id: "canonical-p" });
  expect(transport.pendingOutcome("p")).toBeUndefined();
  expect(calls).toEqual(["cancel_render_job_in_split_project_folder", "read_project_snapshot_from_split_project_folder"]);
  expect(acknowledgements).toBe(2);
});


it("rejects recovery identities exceeding the backend UTF-8 byte limits", () => {
  expect(() => parseServerRequestOutcome({ ...entry, requestId: "α".repeat(65) })).toThrow("recovery");
  expect(() => parseServerRequestOutcome({ ...entry, canonicalProjectId: "α".repeat(513) })).toThrow("recovery");
});

it("refuses a canonical snapshot older than a retained successful project outcome", async () => {
  const receipt = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", status: "succeeded", contentRevision: 6 };
  let acknowledgements = 0;
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt] }));
    if (String(url).endsWith("/ack")) { acknowledgements += 1; return new Response(JSON.stringify({ acknowledged: true })); }
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: "canonical-p", contentRevision: 5 } }));
  } });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toThrow("older than");
  expect(transport.pendingOutcome("p")).toBeDefined();
  expect(acknowledgements).toBe(0);
});

it("does not let a newer local scope marker be cleared by historical server-only recovery", async () => {
  const receipt = { ...entry, requestId: "rpc:1", operation: "save_split_project_to_folder", projectId: "p", status: "interrupted" };
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt] }));
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    persistRemoteOutcomeMarker({ hostOrigin: window.location.origin, requestId: "browser-newer-write", operation: receipt.operation, projectId: "p", phase: "rpc", outcome: "unknown" });
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: "canonical-p", contentRevision: 5 } }));
  } });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toThrow("changed during refresh");
  expect(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")).toContain("browser-newer-write");
  expect(transport.pendingOutcome("p")?.marker.requestId).toBe("rpc:1");
});

it("preserves a newer creation guard when only an older creation is terminal", async () => {
  persistRemoteOutcomeMarker({ hostOrigin: window.location.origin, requestId: "browser-newer-creation", operation: "remote_create_project", phase: "rpc", outcome: "unknown" });
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [{ ...entry, status: "rejected" }] }));
    if (String(url).endsWith("/ack")) return new Response(JSON.stringify({ acknowledged: true }));
    return reply(init);
  } });
  await transport.request("remote_list_projects");
  await expect(transport.request("remote_create_project")).rejects.toMatchObject({ outcome: "unknown", requestId: "browser-newer-creation" });
});

it("rejects backward pagination before accepting its receipts as complete recovery", async () => {
  let requests = 0;
  const client = new ServerOutcomeClient(async () => {
    requests += 1;
    const requestId = requests === 1 ? "browser-z" : "browser-a";
    return new Response(JSON.stringify({ outcomes: [{ ...entry, requestId }], nextCursor: requestId }));
  }, () => "csrf", () => true, 1000);
  await expect(client.list()).rejects.toThrow("recovery");
  expect(requests).toBe(2);
});


it("keeps the project guard when the acknowledgement body does not confirm acceptance", async () => {
  const receipt = { ...entry, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", status: "interrupted" };
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [receipt] }));
    if (String(url).endsWith("/ack")) return new Response(JSON.stringify({ acknowledged: false }));
    if (String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: { id: "canonical-p", contentRevision: 5 } }));
  } });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toThrow("recovery");
  expect(transport.pendingOutcome("p")).toBeDefined();
});


it("reconciles a server-only opaque guard when other origins fill browser marker storage", async () => {
  for (let index=0;index<32;index+=1) persistRemoteOutcomeMarker({ hostOrigin:`https://other-${index}.test`, requestId:`browser-other-${index}`, operation:"save_split_project_to_folder", projectId:`other-p-${index}`, phase:"rpc", outcome:"unknown" });
  const receipt={ ...entry, requestId:"browser-server-only", operation:"save_split_project_to_folder", projectId:"p", status:"interrupted", canonicalProjectId:"canonical-p" };
  const transport=new RemoteTransport({ csrfToken:"csrf", outcomeProtocol:1, fetcher:async (url,init)=> {
    if(String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({outcomes:[receipt]}));
    if(String(url).endsWith("/ack")) return new Response(JSON.stringify({acknowledged:true}));
    if(String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    return new Response(JSON.stringify({requestId:JSON.parse(String(init?.body)).requestId,ok:true,result:{id:"canonical-p",contentRevision:5}}));
  }});
  await transport.request("remote_list_projects");
  expect(transport.pendingOutcome("p")).toBeDefined();
  await expect(transport.reconcileProject("p","canonical-p")).resolves.toEqual({id:"canonical-p",contentRevision:5});
  expect(transport.pendingOutcome("p")).toBeUndefined();
  expect(JSON.parse(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")??"[]")).toHaveLength(32);
});


it("reconciles the server-owned fallback when browser writes fail after reads succeed", async () => {
  const receipt={ ...entry, requestId:"browser-quota-only", operation:"save_split_project_to_folder", projectId:"p", status:"interrupted", canonicalProjectId:"canonical-p" };
  const spy=vi.spyOn(Storage.prototype,"setItem").mockImplementation(()=>{throw new Error("quota");});
  try {
    const transport=new RemoteTransport({csrfToken:"csrf",outcomeProtocol:1,fetcher:async(url,init)=>{
      if(String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({outcomes:[receipt]}));
      if(String(url).endsWith("/ack")) return new Response(JSON.stringify({acknowledged:true}));
      if(String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
      return new Response(JSON.stringify({requestId:JSON.parse(String(init?.body)).requestId,ok:true,result:{id:"canonical-p",contentRevision:5}}));
    }});
    await transport.request("remote_list_projects");
    await expect(transport.reconcileProject("p","canonical-p")).resolves.toEqual({id:"canonical-p",contentRevision:5});
    expect(transport.pendingOutcome("p")).toBeUndefined();
  } finally {spy.mockRestore();}
});

it("preserves a newer local marker during server-only opaque reconciliation", async () => {
  for(let index=0;index<32;index+=1) persistRemoteOutcomeMarker({hostOrigin:`https://other-${index}.test`,requestId:`browser-other-${index}`,operation:"save_split_project_to_folder",projectId:`other-p-${index}`,phase:"rpc",outcome:"unknown"});
  const receipt={...entry,requestId:"browser-server-only",operation:"save_split_project_to_folder",projectId:"p",status:"interrupted",canonicalProjectId:"canonical-p"};
  let acknowledgements=0;
  const transport=new RemoteTransport({csrfToken:"csrf",outcomeProtocol:1,fetcher:async(url,init)=>{
    if(String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({outcomes:[receipt]}));
    if(String(url).endsWith("/ack")) {acknowledgements+=1;return new Response(JSON.stringify({acknowledged:true}));}
    if(String(url).includes("/request-outcomes/")) return new Response(JSON.stringify(receipt));
    if(JSON.parse(String(init?.body)).operation==="read_project_snapshot_from_split_project_folder") {
      const markers=JSON.parse(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")??"[]") as unknown[];
      window.sessionStorage.setItem("video-creater.remotePendingOutcomes.v1",JSON.stringify(markers.slice(1)));
      persistRemoteOutcomeMarker({hostOrigin:window.location.origin,requestId:"browser-newer",operation:"save_split_project_to_folder",projectId:"p",phase:"rpc",outcome:"unknown"});
    }
    return new Response(JSON.stringify({requestId:JSON.parse(String(init?.body)).requestId,ok:true,result:{id:"canonical-p",contentRevision:5}}));
  }});
  await transport.request("remote_list_projects");
  await expect(transport.reconcileProject("p","canonical-p")).rejects.toThrow("changed during refresh");
  expect(acknowledgements).toBe(0);
  expect(window.sessionStorage.getItem("video-creater.remotePendingOutcomes.v1")).toContain("browser-newer");
});
