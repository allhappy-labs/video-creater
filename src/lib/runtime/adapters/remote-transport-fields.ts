import { withRemoteDeadline } from "./remote-deadline";
import type { BackendInput, RemoteErrorCode } from "../backend-transport";
import { remoteProjectRevision } from "./remote-project-access";

export type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export interface RpcResponse<Result> {
  readonly requestId: string; readonly ok: boolean; readonly result?: Result;
  readonly error?: { readonly code?: RemoteErrorCode; readonly message?: string; readonly retryAfterMs?: number };
}
export interface RemoteEvent { readonly sequence: number; readonly topic: string; readonly payload: unknown; }
export const rpcErrorCodes = new Set<RemoteErrorCode>(["invalid_request", "unauthorized", "forbidden", "not_found", "conflict", "rate_limited", "busy", "internal", "outcome_unknown", "outcome_expired"]);

export function randomId(): string {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  const bytes = new Uint8Array(16);
  if (globalThis.crypto?.getRandomValues) globalThis.crypto.getRandomValues(bytes);
  else for (let index = 0; index < bytes.length; index += 1) bytes[index] = Math.floor(Math.random() * 256);
  bytes[6] = (bytes[6]! & 15) | 64;
  bytes[8] = (bytes[8]! & 63) | 128;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export function stringField(input: BackendInput, key: string): string | undefined {
  return typeof input[key] === "string" ? input[key] : undefined;
}

function numberField(input: BackendInput, key: string): number | undefined {
  return typeof input[key] === "number" ? input[key] : undefined;
}

export function projectIdFor(input: BackendInput): string | undefined {
  return stringField(input, "projectDir") ?? stringField(input, "projectId");
}

export function expectedRevisionFor(input: BackendInput, projectId: string | undefined): number | undefined {
  const supplied = numberField(input, "expectedRevision");
  if (supplied !== undefined) return supplied;
  return projectId ? remoteProjectRevision(projectId) : undefined;
}

export function stringFieldFromUnknown(value: unknown, key: string): string | undefined {
  return typeof value === "object" && value !== null && key in value && typeof (value as Record<string, unknown>)[key] === "string"
    ? (value as Record<string, string>)[key]
    : undefined;
}

export function projectRevisionFromResult(value: unknown): number | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const record = value as Record<string, unknown>;
  if (typeof record.contentRevision === "number") return record.contentRevision;
  if (typeof record.project === "object" && record.project !== null) {
    const revision = (record.project as Record<string, unknown>).contentRevision;
    if (typeof revision === "number") return revision;
  }
  return undefined;
}

export async function remoteArtifactUrl(projectId: string, artifactId: string, fetcher: Fetcher, csrfToken: string, deadline: number): Promise<string> {
  const { response, body } = await withRemoteDeadline(deadline, async (signal) => {
    const response = await fetcher("/api/v1/resource-tickets/artifact", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
        "x-csrf-token": csrfToken,
      },
      body: JSON.stringify({ projectId, artifactId }),
      signal,
    });
    return { response, body: await response.json() as { readonly url?: string; readonly message?: string } };
  });
  if (!response.ok || !body.url || !/^\/api\/v1\/artifacts\/[a-f0-9]{32}$/.test(body.url)) {
    throw new Error(body.message ?? "Export download is not available.");
  }
  return body.url;
}
