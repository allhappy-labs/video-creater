import { withRemoteDeadline } from "./remote-deadline";
import { safeRemoteCanonicalProjectId } from "./remote-outcome-storage";

export type OutcomeFetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export interface ServerRequestOutcome {
  readonly requestId: string;
  readonly operation: string;
  readonly status: "pending" | "interrupted" | "succeeded" | "rejected";
  readonly projectId?: string;
  readonly canonicalProjectId?: string;
  readonly catalogProjectId?: string;
  readonly contentRevision?: number;
  readonly issuedAt: number;
  readonly acknowledged: boolean;
}
/** Authenticated absence plus permanent replay fencing; no original outcome is inferred. */
export class ExpiredServerRequestOutcome {
  readonly kind = "expired_fence";
  constructor(readonly requestId: string) {}
}
export class RemoteOutcomeCapacityError extends Error {
  constructor(message = "Too many host operations remain unconfirmed. Refresh those projects before editing. The request was not sent.") { super(message); }
}
class RemoteOutcomeUnpersistableError extends RemoteOutcomeCapacityError {
  constructor() { super("A historical host operation needs explicit recovery before editing. The request was not sent."); }
}
/** Terminal creations need no snapshot. Other receipts retain guards in bounded batches. */
export function restoreServerOutcomes(receipts: readonly ServerRequestOutcome[], creation: (receipt: ServerRequestOutcome) => void, unresolved: (receipt: ServerRequestOutcome) => void): void {
  const retained: ServerRequestOutcome[] = [];
  for (const receipt of receipts) {
    if (receipt.acknowledged) continue;
    if (receipt.operation === "remote_create_project" && ["succeeded", "rejected"].includes(receipt.status)) creation(receipt);
    else retained.push(receipt);
  }
  for (const receipt of retained.slice(0, 32)) unresolved(receipt);
  if (retained.length > 32) throw new RemoteOutcomeCapacityError();
  if (retained.some((receipt) => !opaque(receipt.requestId))) throw new RemoteOutcomeUnpersistableError();
}

const receiptKeys = ["requestId", "operation", "status", "projectId", "canonicalProjectId", "catalogProjectId", "contentRevision", "issuedAt", "acknowledged"];
const message = "The host’s recovery records could not be verified. Reconnect or refresh before editing. The request was not sent.";
function object(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error(message);
  return value as Record<string, unknown>;
}
function opaque(value: unknown): value is string {
  return typeof value === "string" && safeRemoteCanonicalProjectId(value) !== undefined;
}
function boundedIdentity(value: unknown, maximumBytes: number): value is string {
  if (typeof value !== "string" || value.length === 0 || new TextEncoder().encode(value).length > maximumBytes) return false;
  try { encodeURIComponent(value); return true; } catch { return false; }
}
function requestIdentity(value: unknown): value is string { return boundedIdentity(value, 128); }
function canonicalIdentity(value: unknown): value is string { return boundedIdentity(value, 1024); }
function cursorAfter(next: string, previous: string): boolean {
  const left = new TextEncoder().encode(next);
  const right = new TextEncoder().encode(previous);
  for (let index = 0; index < Math.min(left.length, right.length); index += 1) {
    if (left[index] !== right[index]) return left[index]! > right[index]!;
  }
  return left.length > right.length;
}
export function parseServerRequestOutcome(value: unknown): ServerRequestOutcome {
  const record = object(value);
  if (!Object.keys(record).every((key) => receiptKeys.includes(key)) || !requestIdentity(record.requestId) ||
    typeof record.operation !== "string" || !/^[a-z][a-z0-9_]{0,127}$/.test(record.operation) ||
    typeof record.status !== "string" || !["pending", "interrupted", "succeeded", "rejected"].includes(record.status) ||
    typeof record.acknowledged !== "boolean" || !Number.isSafeInteger(record.issuedAt) || (record.issuedAt as number) < 0 ||
    ["projectId", "catalogProjectId"].some((key) => record[key] !== undefined && !opaque(record[key])) ||
    record.canonicalProjectId !== undefined && !canonicalIdentity(record.canonicalProjectId) ||
    record.contentRevision !== undefined && (!Number.isSafeInteger(record.contentRevision) || (record.contentRevision as number) < 0) ||
    record.operation === "remote_create_project" && record.status === "succeeded" && (!opaque(record.catalogProjectId) || !canonicalIdentity(record.canonicalProjectId))
  ) throw new Error(message);
  return record as unknown as ServerRequestOutcome;
}

/** Authenticated compact receipts only; canonical snapshots remain separate RPC reads. */
export class ServerOutcomeClient {
  private readonly acknowledgements: string[] = [];
  private readonly queuedAcknowledgements = new Set<string>();
  private acknowledging = false;
  constructor(private readonly fetcher: OutcomeFetcher, private readonly csrfToken: () => string, private readonly current: () => boolean, private readonly deadline: number) {}

  private async request(path: string, acknowledge = false, allowExpiry = false): Promise<unknown> {
    if (!this.current()) throw new Error("The host connection changed during recovery.");
    return withRemoteDeadline(this.deadline, async (signal) => {
      const response = await this.fetcher(path, {
        method: acknowledge ? "POST" : "GET", credentials: "same-origin", cache: "no-store",
        headers: acknowledge ? { accept: "application/json", "content-type": "application/json", "x-csrf-token": this.csrfToken() } : { accept: "application/json" },
        ...(acknowledge ? { body: "{}" } : {}), signal,
      });
      const expired = allowExpiry && response.status === 410;
      if (!response.ok && !expired) throw new Error(message);
      const body: unknown = await response.json();
      if (!this.current()) throw new Error("The host connection changed during recovery.");
      if (expired) {
        const record = object(body);
        if (!Object.keys(record).every((key) => ["error", "requestId", "replayFenced"].includes(key)) || record.error !== "outcome_expired" || record.replayFenced !== true || !requestIdentity(record.requestId) || !/^browser-v2-\d+-[A-Za-z0-9._-]+$/.test(record.requestId)) throw new Error(message);
        return new ExpiredServerRequestOutcome(record.requestId);
      }
      return body;
    });
  }

  async list(): Promise<readonly ServerRequestOutcome[]> {
    const outcomes: ServerRequestOutcome[] = [];
    const seen = new Set<string>();
    let cursor: string | undefined;
    for (let page = 0; page < 70; page += 1) {
      const body = object(await this.request(`/api/v1/request-outcomes${cursor ? `?after=${encodeURIComponent(cursor)}` : ""}`));
      if (!Object.keys(body).every((key) => ["outcomes", "nextCursor"].includes(key)) || !Array.isArray(body.outcomes) || body.outcomes.length > 256 || body.nextCursor !== undefined && body.nextCursor !== null && !requestIdentity(body.nextCursor)) throw new Error(message);
      for (const value of body.outcomes) {
        const receipt = parseServerRequestOutcome(value);
        if (seen.has(receipt.requestId)) throw new Error(message);
        seen.add(receipt.requestId);
        outcomes.push(receipt);
      }
      if (body.nextCursor === undefined || body.nextCursor === null) return outcomes;
      if (cursor !== undefined && !cursorAfter(body.nextCursor as string, cursor) || !seen.has(body.nextCursor as string)) throw new Error(message);
      cursor = body.nextCursor as string;
    }
    throw new Error(message);
  }

  async get(requestId: string): Promise<ServerRequestOutcome | ExpiredServerRequestOutcome> {
    if (!requestIdentity(requestId)) throw new Error(message);
    const value = await this.request(`/api/v1/request-outcomes/${encodeURIComponent(requestId)}`, false, true);
    const outcome = value instanceof ExpiredServerRequestOutcome ? value : parseServerRequestOutcome(value);
    if (outcome.requestId !== requestId) throw new Error(message);
    return outcome;
  }

  async acknowledge(requestId: string): Promise<void> {
    if (!requestIdentity(requestId)) throw new Error(message);
    const body = object(await this.request(`/api/v1/request-outcomes/${encodeURIComponent(requestId)}/ack`, true));
    if (body.acknowledged !== true || !Object.keys(body).every((key) => key === "acknowledged")) throw new Error(message);
  }

  /** Observational ACKs never change an already known RPC result. */
  bestEffortAcknowledge(requestId: string): void {
    if (this.queuedAcknowledgements.has(requestId) || this.queuedAcknowledgements.size >= 256) return;
    this.queuedAcknowledgements.add(requestId);
    this.acknowledgements.push(requestId);
    if (!this.acknowledging) void this.drainAcknowledgements();
  }

  private async drainAcknowledgements(): Promise<void> {
    this.acknowledging = true;
    while (this.acknowledgements.length) {
      const requestId = this.acknowledgements.shift()!;
      try { await this.acknowledge(requestId); } catch { /* unresolved server receipts remain available */ }
      this.queuedAcknowledgements.delete(requestId);
    }
    this.acknowledging = false;
  }
}
