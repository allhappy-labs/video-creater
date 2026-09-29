import type {
  BackendInput,
  BackendTransport,
  BackendUnlisten,
} from "../backend-transport";
import { cacheMediaTicketsForResult, remoteResourceUrl } from "./remote-resource-cache";
import { remoteCsrfToken, setRemoteCsrfToken } from "./remote-credentials";
import {
  remoteEditorLeaseToken,
  remoteProjectRevision,
  setRemoteProjectAccess,
  setRemoteProjectRevision,
  setRemoteTakeoverHandler,
} from "./remote-project-access";

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

interface RemoteTransportOptions {
  readonly csrfToken: string;
  readonly fetcher?: Fetcher;
  readonly webSocketFactory?: (url: string, protocols: string[]) => WebSocket;
  readonly snapshotRequiredHandler?: () => void;
}

interface RpcResponse<Result> {
  readonly requestId: string;
  readonly ok: boolean;
  readonly result?: Result;
  readonly error?: { readonly message?: string };
}

interface RemoteEvent {
  readonly sequence: number;
  readonly topic: string;
  readonly payload: unknown;
}

export class RemoteTransport implements BackendTransport {
  readonly kind = "remote" as const;
  private readonly fetcher: Fetcher;
  private readonly listeners = new Map<string, Set<(payload: unknown) => void>>();
  private socket: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;
  private lastSequence = 0;
  private stopped = false;
  private readonly projectQueues = new Map<string, Promise<void>>();

  constructor(private readonly options: RemoteTransportOptions) {
    this.fetcher = options.fetcher ?? globalThis.fetch.bind(globalThis);
    setRemoteCsrfToken(options.csrfToken);
    setRemoteTakeoverHandler((projectId) => this.acquireProjectLease(projectId, true));
  }

  request<Result>(operation: string, input: BackendInput = {}): Promise<Result> {
    const projectId = projectIdFor(input);
    if (!projectId || operation.startsWith("cancel_")) {
      return this.performRequest<Result>(operation, input, projectId);
    }
    const previous = this.projectQueues.get(projectId) ?? Promise.resolve();
    const result = previous
      .catch(() => undefined)
      .then(() => this.performRequest<Result>(operation, input, projectId));
    const settled = result.then(() => undefined, () => undefined);
    this.projectQueues.set(projectId, settled);
    void settled.finally(() => {
      if (this.projectQueues.get(projectId) === settled) this.projectQueues.delete(projectId);
    });
    return result;
  }

  private async performRequest<Result>(
    operation: string,
    input: BackendInput,
    projectId: string | undefined,
  ): Promise<Result> {
    const requestId = `browser-${randomId()}`;
    if (
      projectId &&
      operation !== "load_split_project_from_folder" &&
      !operation.startsWith("cancel_")
    ) {
      await this.acquireProjectLease(projectId, false);
    }
    const response = await this.fetcher("/api/v1/rpc", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
        "x-csrf-token": this.csrfToken(),
      },
      body: JSON.stringify({
        requestId,
        operation,
        projectId,
        expectedRevision: expectedRevisionFor(input, projectId),
        editorLeaseToken: stringField(input, "editorLeaseToken") ?? (projectId ? remoteEditorLeaseToken(projectId) : undefined),
        payload: input,
      }),
    });
    const body = await response.json() as RpcResponse<Result>;
    if (!response.ok || !body.ok) {
      throw new Error(body.error?.message ?? "Remote operation failed");
    }
    if (body.requestId !== requestId) {
      throw new Error("Remote response did not match the request");
    }
    const result = body.result as Result;
    if (projectId) {
      const revision = projectRevisionFromResult(result);
      if (revision !== undefined) setRemoteProjectRevision(projectId, revision);
      await cacheMediaTicketsForResult(projectId, result, this.csrfToken(), this.fetcher);
    }
    if (operation === "load_split_project_from_folder" && projectId) {
      await this.acquireProjectLease(projectId, false);
    }
    if (operation === "remote_create_project") {
      const createdProjectId = stringFieldFromUnknown(result, "catalogProjectId");
      if (createdProjectId) {
        const revision = projectRevisionFromResult(result);
        if (revision !== undefined) setRemoteProjectRevision(createdProjectId, revision);
        await this.acquireProjectLease(createdProjectId, false);
      }
    }
    return result;
  }

  async listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten> {
    let handlers = this.listeners.get(event);
    if (!handlers) {
      handlers = new Set();
      this.listeners.set(event, handlers);
    }
    const listener = handler as (payload: unknown) => void;
    handlers.add(listener);
    this.stopped = false;
    this.ensureSocket();
    return () => {
      handlers?.delete(listener);
      if (handlers?.size === 0) this.listeners.delete(event);
      if (this.listeners.size === 0) this.disconnectEvents();
    };
  }

  mediaUrl(path: string): string {
    return remoteResourceUrl(path);
  }

  async artifactUrl(projectId: string, artifactId: string): Promise<string> {
    const response = await this.fetcher("/api/v1/resource-tickets/artifact", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
        "x-csrf-token": this.csrfToken(),
      },
      body: JSON.stringify({ projectId, artifactId }),
    });
    const body = await response.json() as { readonly url?: string; readonly message?: string };
    if (!response.ok || !body.url || !/^\/api\/v1\/artifacts\/[a-f0-9]{32}$/.test(body.url)) {
      throw new Error(body.message ?? "Export download is not available.");
    }
    return body.url;
  }

  private async acquireProjectLease(projectId: string, takeover: boolean): Promise<void> {
    const response = await this.fetcher(`/api/v1/projects/${encodeURIComponent(projectId)}/lease`, {
      method: "POST",
      credentials: "same-origin",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
        "x-csrf-token": this.csrfToken(),
      },
      body: JSON.stringify({ takeover }),
    });
    const body = await response.json() as {
      readonly mode?: "editor" | "readOnly";
      readonly editorLeaseToken?: string;
      readonly expiresAt?: number;
      readonly editorDisplayName?: string;
      readonly message?: string;
    };
    if (response.status === 409 && body.mode === "readOnly") {
      setRemoteProjectAccess(projectId, {
        mode: "readOnly",
        editorDisplayName: body.editorDisplayName ?? "Another device",
      });
      return;
    }
    if (!response.ok || body.mode !== "editor" || !body.editorLeaseToken || typeof body.expiresAt !== "number") {
      throw new Error(body.message ?? "Editing access could not be acquired.");
    }
    setRemoteProjectAccess(projectId, {
      mode: "editing",
      token: body.editorLeaseToken,
      expiresAt: body.expiresAt,
    });
  }

  private ensureSocket(): void {
    if (this.socket || this.reconnectTimer || this.listeners.size === 0) return;
    const factory = this.options.webSocketFactory ?? ((url, protocols) => new WebSocket(url, protocols));
    const base = typeof window === "undefined"
      ? "ws://localhost"
      : `${window.location.protocol === "https:" ? "wss:" : "ws:"}//${window.location.host}`;
    const socket = factory(`${base}/api/v1/events`, [
      "vc-events",
      `vc-csrf.${this.csrfToken()}`,
      `vc-resume.${this.lastSequence}`,
    ]);
    this.socket = socket;
    socket.onopen = () => { this.reconnectAttempt = 0; };
    socket.onmessage = (message) => this.deliverEvent(message.data);
    socket.onclose = () => {
      if (this.socket === socket) this.socket = null;
      if (!this.stopped && this.listeners.size > 0) this.scheduleReconnect();
    };
    socket.onerror = () => socket.close();
  }

  private csrfToken(): string {
    return remoteCsrfToken(this.options.csrfToken);
  }

  private deliverEvent(raw: unknown): void {
    if (typeof raw !== "string") return;
    try {
      const event = JSON.parse(raw) as RemoteEvent | { type: "snapshotRequired" };
      if (!("sequence" in event) || typeof event.sequence !== "number") {
        if ("type" in event && event.type === "snapshotRequired") {
          if (this.options.snapshotRequiredHandler) this.options.snapshotRequiredHandler();
          else if (typeof window !== "undefined") window.location.reload();
        }
        return;
      }
      this.lastSequence = Math.max(this.lastSequence, event.sequence);
      for (const handler of this.listeners.get(event.topic) ?? []) handler(event.payload);
    } catch {
      // A malformed or future-version event cannot be allowed to break the stream.
    }
  }

  private scheduleReconnect(): void {
    const delays = [250, 500, 1_000, 2_000, 5_000];
    const delay = delays[Math.min(this.reconnectAttempt, delays.length - 1)];
    this.reconnectAttempt += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.ensureSocket();
    }, delay);
  }

  private disconnectEvents(): void {
    this.stopped = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.socket?.close(1000, "no listeners");
    this.socket = null;
  }
}

function randomId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function stringField(input: BackendInput, key: string): string | undefined {
  return typeof input[key] === "string" ? input[key] : undefined;
}

function numberField(input: BackendInput, key: string): number | undefined {
  return typeof input[key] === "number" ? input[key] : undefined;
}

function projectIdFor(input: BackendInput): string | undefined {
  return stringField(input, "projectDir") ?? stringField(input, "projectId");
}

function expectedRevisionFor(input: BackendInput, projectId: string | undefined): number | undefined {
  const supplied = numberField(input, "expectedRevision");
  if (supplied !== undefined) return supplied;
  return projectId ? remoteProjectRevision(projectId) : undefined;
}

function stringFieldFromUnknown(value: unknown, key: string): string | undefined {
  return typeof value === "object" && value !== null && key in value && typeof (value as Record<string, unknown>)[key] === "string"
    ? (value as Record<string, string>)[key]
    : undefined;
}

function projectRevisionFromResult(value: unknown): number | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const record = value as Record<string, unknown>;
  if (typeof record.contentRevision === "number") return record.contentRevision;
  if (typeof record.project === "object" && record.project !== null) {
    const revision = (record.project as Record<string, unknown>).contentRevision;
    if (typeof revision === "number") return revision;
  }
  return undefined;
}
