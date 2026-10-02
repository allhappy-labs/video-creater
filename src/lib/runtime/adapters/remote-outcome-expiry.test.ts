import { beforeEach, expect, it } from "vitest";
import { RemoteTransport } from "./remote-transport";
import { loadRemoteOutcomeMarkers, persistRemoteOutcomeMarker } from "./remote-outcome-storage";

const requestId = "browser-v2-1-expired-receipt";
const marker = { requestId, operation: "save_split_project_to_folder", projectId: "p", canonicalProjectId: "canonical-p", phase: "rpc", outcome: "unknown" } as const;
const attestation = { error: "outcome_expired", requestId, replayFenced: true };
beforeEach(() => { window.sessionStorage.clear(); persistRemoteOutcomeMarker({ ...marker, hostOrigin: window.location.origin }); });

function fixture(options: { lookup?: Response; ack?: Response; snapshot?: unknown } = {}) {
  const calls: string[] = [];
  const transport = new RemoteTransport({ csrfToken: "csrf", outcomeProtocol: 1, fetcher: async (url, init) => {
    calls.push(String(url));
    if (String(url).endsWith("/request-outcomes")) return new Response(JSON.stringify({ outcomes: [], nextCursor: null }));
    if (String(url).endsWith("/ack")) return options.ack?.clone() ?? new Response(JSON.stringify({ acknowledged: true }));
    if (String(url).includes("/request-outcomes/")) return options.lookup?.clone() ?? new Response(JSON.stringify(attestation), { status: 410 });
    return new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: options.snapshot ?? { id: "canonical-p", contentRevision: 7 } }));
  } });
  return { transport, calls };
}

it("reconciles an expired fenced project identity through explicit canonical read and confirmed ACK", async () => {
  const { transport, calls } = fixture();
  await expect(transport.reconcileProject("p", "canonical-p")).resolves.toEqual({ id: "canonical-p", contentRevision: 7 });
  expect(calls).toEqual(["/api/v1/request-outcomes", `/api/v1/request-outcomes/${requestId}`, "/api/v1/rpc", `/api/v1/request-outcomes/${requestId}/ack`]);
  expect(loadRemoteOutcomeMarkers(window.location.origin)).toEqual([]);
  expect(transport.pendingOutcome("p")).toBeUndefined();
});

it("requires independently supplied project identity before an expired receipt can be reconciled", async () => {
  const { transport, calls } = fixture();
  await expect(transport.reconcileProject("p")).rejects.toThrow("Open the project");
  expect(calls).not.toContain("/api/v1/rpc");
  expect(transport.pendingOutcome("p")).toBeDefined();
});

it.each([
  new Response("{}", { status: 404 }), new Response("{}", { status: 503 }),
  new Response(JSON.stringify({ ...attestation, replayFenced: false }), { status: 410 }),
  new Response(JSON.stringify({ ...attestation, requestId: "browser-v2-1-another" }), { status: 410 }),
  new Response(JSON.stringify({ ...attestation, projectId: "p" }), { status: 410 }),
  new Response(JSON.stringify(attestation), { status: 200 }),
])("keeps the guard without a matching strict authenticated expiry attestation", async (lookup) => {
  const { transport, calls } = fixture({ lookup });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toThrow();
  expect(calls).not.toContain("/api/v1/rpc");
  expect(transport.pendingOutcome("p")).toBeDefined();
});

it.each([new Response("{}", { status: 409 }), new Response(JSON.stringify({ acknowledged: false }))])("keeps an expired guard when ACK does not confirm the fence", async (ack) => {
  const { transport } = fixture({ ack });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toThrow();
  expect(transport.pendingOutcome("p")).toBeDefined();
});

it("never acknowledges an expired fence after the canonical read returns a different project", async () => {
  const { transport, calls } = fixture({ snapshot: { id: "replacement", contentRevision: 7 } });
  await expect(transport.reconcileProject("p", "canonical-p")).rejects.toThrow("different project");
  expect(calls.some((url) => url.endsWith("/ack"))).toBe(false);
  expect(transport.pendingOutcome("p")).toBeDefined();
});
