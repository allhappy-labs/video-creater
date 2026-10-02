const REMOTE_PROTOCOL_VERSION = 1;

export type RemoteSessionState =
  | { readonly kind: "unpaired" }
  | {
      readonly kind: "connected";
      readonly sessionId: string;
      readonly displayName: string;
      readonly hostLabel: string;
      readonly csrfToken: string;
      readonly outcomeProtocol?: 1;
    }
  | { readonly kind: "incompatible"; readonly hostProtocolVersion: number }
  | { readonly kind: "unavailable" }
  | { readonly kind: "revoked" }
  | { readonly kind: "reconnecting"; readonly displayName?: string }
  | { readonly kind: "readOnly"; readonly editorDisplayName: string }
  | { readonly kind: "takeover"; readonly previousEditorDisplayName: string };

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

interface SessionPayload {
  readonly authenticated?: boolean;
  readonly sessionId?: unknown;
  readonly displayName?: unknown;
  readonly csrfToken?: unknown;
  readonly protocolVersion?: unknown;
  readonly hostLabel?: unknown;
  readonly outcomeProtocol?: unknown;
}

export interface RemoteDeviceSession {
  readonly sessionId: string;
  readonly displayName: string;
  readonly identity?: string;
  readonly createdAt: number;
  readonly expiresAt: number;
  readonly revoked: boolean;
  readonly current: boolean;
}

export async function discoverRemoteSession(
  fetcher: Fetcher = fetch,
): Promise<RemoteSessionState> {
  try {
    const response = await fetcher("/api/v1/session", {
      method: "GET",
      credentials: "same-origin",
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (response.status === 401) return { kind: "unpaired" };
    if (!response.ok) return { kind: "unavailable" };
    return connectedState(await response.json() as SessionPayload);
  } catch {
    return { kind: "unavailable" };
  }
}

export async function pairRemoteDevice(
  code: string,
  displayName: string,
  fetcher: Fetcher = fetch,
): Promise<RemoteSessionState> {
  const response = await fetcher("/api/v1/pair", {
    method: "POST",
    credentials: "same-origin",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify({ code, displayName }),
  });
  if (!response.ok) {
    throw new Error(response.status === 401
      ? "That pairing code was not accepted."
      : "This device could not be paired.");
  }
  return connectedState(await response.json() as SessionPayload);
}

export async function listRemoteDeviceSessions(
  fetcher: Fetcher = fetch,
): Promise<readonly RemoteDeviceSession[]> {
  const response = await fetcher("/api/v1/admin/sessions", {
    method: "GET",
    credentials: "same-origin",
    cache: "no-store",
    headers: { accept: "application/json" },
  });
  if (!response.ok) throw new Error("Paired devices could not be loaded.");
  const payload = await response.json() as {
    readonly currentSessionId?: unknown;
    readonly sessions?: unknown;
  };
  if (!Array.isArray(payload.sessions)) return [];
  return payload.sessions.flatMap((value): RemoteDeviceSession[] => {
    if (typeof value !== "object" || value === null) return [];
    const record = value as Record<string, unknown>;
    if (
      typeof record.sessionId !== "string" ||
      typeof record.displayName !== "string" ||
      typeof record.createdAt !== "number" ||
      typeof record.expiresAt !== "number" ||
      typeof record.revoked !== "boolean"
    ) return [];
    return [{
      sessionId: record.sessionId,
      displayName: record.displayName,
      ...(typeof record.identity === "string" ? { identity: record.identity } : {}),
      createdAt: record.createdAt,
      expiresAt: record.expiresAt,
      revoked: record.revoked,
      current: payload.currentSessionId === record.sessionId,
    }];
  });
}

export async function revokeRemoteDeviceSession(
  sessionId: string,
  csrfToken: string,
  fetcher: Fetcher = fetch,
): Promise<void> {
  const response = await fetcher(
    `/api/v1/admin/sessions/${encodeURIComponent(sessionId)}/revoke`,
    {
      method: "POST",
      credentials: "same-origin",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
        "x-csrf-token": csrfToken,
      },
      body: "{}",
    },
  );
  if (!response.ok) throw new Error("The paired device could not be revoked.");
}

function connectedState(payload: SessionPayload): RemoteSessionState {
  if (payload.protocolVersion !== REMOTE_PROTOCOL_VERSION) {
    return {
      kind: "incompatible",
      hostProtocolVersion: typeof payload.protocolVersion === "number"
        ? payload.protocolVersion
        : -1,
    };
  }
  if (
    payload.authenticated !== true ||
    typeof payload.sessionId !== "string" ||
    typeof payload.csrfToken !== "string"
  ) {
    return { kind: "unpaired" };
  }
  return {
    kind: "connected",
    sessionId: payload.sessionId,
    displayName: typeof payload.displayName === "string"
      ? payload.displayName
      : "Paired device",
    hostLabel: typeof payload.hostLabel === "string"
      ? payload.hostLabel
      : "Video Creater host",
    csrfToken: payload.csrfToken,
    ...(payload.outcomeProtocol === 1 ? { outcomeProtocol: 1 as const } : {}),
  };
}
