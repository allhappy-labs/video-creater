/** Recovery markers only: never RPC payloads, credentials, media paths or project snapshots. */
export interface RemoteOutcomeMarker {
  readonly hostOrigin: string;
  readonly requestId: string;
  readonly operation: string;
  readonly projectId?: string | undefined;
  readonly canonicalProjectId?: string | undefined;
  readonly phase: "rpc";
  readonly outcome: "pending" | "unknown";
}

const storageKey = "video-creater.remotePendingOutcomes.v1";
const maximumMarkers = 32;
const maximumBytes = 24 * 1024;
const opaqueId = /^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$/;
const storageMessage = "Browser recovery storage is unavailable or damaged. Enable browser storage or restore this tab’s recovery data before editing. The request was not sent.";

/** Historical project IDs remain valid to the backend; only safe IDs belong in metadata. */
export function safeRemoteCanonicalProjectId(value: string | undefined): string | undefined {
  return value !== undefined && opaqueId.test(value) ? value : undefined;
}

function validOrigin(value: unknown): value is string {
  if (typeof value !== "string" || value.length > 512) return false;
  try { const url = new URL(value); return ["http:", "https:"].includes(url.protocol) && url.origin === value; }
  catch { return false; }
}

function validMarker(value: unknown): value is RemoteOutcomeMarker {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return Object.keys(record).every((key) => ["hostOrigin", "requestId", "operation", "projectId", "canonicalProjectId", "phase", "outcome"].includes(key)) &&
    validOrigin(record.hostOrigin) && typeof record.requestId === "string" && opaqueId.test(record.requestId) &&
    typeof record.operation === "string" && /^[a-z][a-z0-9_]{0,127}$/.test(record.operation) &&
    (record.projectId === undefined || typeof record.projectId === "string" && opaqueId.test(record.projectId)) &&
    (record.canonicalProjectId === undefined || typeof record.canonicalProjectId === "string" && opaqueId.test(record.canonicalProjectId)) &&
    record.phase === "rpc" && typeof record.outcome === "string" && ["pending", "unknown"].includes(record.outcome);
}

function readMarkers(): RemoteOutcomeMarker[] {
  try {
    const raw = globalThis.sessionStorage.getItem(storageKey);
    if (raw === null) return [];
    if (raw.length > maximumBytes) throw new Error();
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length > maximumMarkers || !parsed.every(validMarker)) throw new Error();
    const scopes = new Set(parsed.map((marker) => JSON.stringify([marker.hostOrigin, marker.projectId])));
    if (scopes.size !== parsed.length) throw new Error();
    return parsed;
  } catch { throw new Error(storageMessage); }
}

function writeMarkers(markers: RemoteOutcomeMarker[]): void {
  try {
    const raw = JSON.stringify(markers);
    if (raw.length > maximumBytes) throw new Error();
    globalThis.sessionStorage.setItem(storageKey, raw);
    // A storage implementation silently ignoring writes is not durable recovery.
    if (globalThis.sessionStorage.getItem(storageKey) !== raw) throw new Error();
  } catch { throw new Error(storageMessage); }
}

export function remoteOutcomeHostOrigin(): string {
  const origin = globalThis.location?.origin;
  if (!validOrigin(origin)) throw new Error(storageMessage);
  return origin;
}

export function loadRemoteOutcomeMarkers(hostOrigin: string): RemoteOutcomeMarker[] {
  return readMarkers().filter((marker) => marker.hostOrigin === hostOrigin);
}

export function persistRemoteOutcomeMarker(marker: RemoteOutcomeMarker): void {
  if (!validMarker(marker)) throw new Error(storageMessage);
  const markers = readMarkers();
  const existing = markers.findIndex((entry) => entry.hostOrigin === marker.hostOrigin && entry.projectId === marker.projectId);
  if (existing >= 0 && markers[existing]?.requestId !== marker.requestId) throw new Error("The last operation may have completed. Refresh the project before editing again. The request was not sent.");
  if (existing >= 0) markers[existing] = marker;
  else {
    if (markers.length >= maximumMarkers) throw new Error("Refresh unconfirmed projects before starting more work. The request was not sent.");
    markers.push(marker);
  }
  writeMarkers(markers);
}

export function clearRemoteOutcomeMarker(hostOrigin: string, projectId: string | undefined, requestId: string): boolean {
  const markers = readMarkers();
  const index = markers.findIndex((marker) => marker.hostOrigin === hostOrigin && marker.projectId === projectId && marker.requestId === requestId);
  if (index === -1) return false;
  markers.splice(index, 1);
  writeMarkers(markers);
  return true;
}

/** Server-only historical IDs remain in memory; a newer local scope always wins. */
export function remoteOutcomeMarkerIsCurrent(marker: RemoteOutcomeMarker, allowAbsent = false): boolean {
  const scope = readMarkers().filter((entry) => entry.hostOrigin === marker.hostOrigin && entry.projectId === marker.projectId);
  return safeRemoteCanonicalProjectId(marker.requestId) !== undefined
    ? scope.some((entry) => entry.requestId === marker.requestId) || allowAbsent && scope.length === 0
    : scope.length === 0;
}

export function clearRestoredRemoteOutcomeMarker(marker: RemoteOutcomeMarker, allowAbsent = false): boolean {
  if (allowAbsent && !readMarkers().some((entry) => entry.hostOrigin === marker.hostOrigin && entry.projectId === marker.projectId)) return true;
  return safeRemoteCanonicalProjectId(marker.requestId) !== undefined
    ? clearRemoteOutcomeMarker(marker.hostOrigin, marker.projectId, marker.requestId)
    : remoteOutcomeMarkerIsCurrent(marker);
}
