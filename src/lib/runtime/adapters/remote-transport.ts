import type {
  BackendInput,
  BackendTransport,
  BackendUnlisten,
} from "../backend-transport";
import { RemoteOperationError, type RemoteRequestPhase } from "../backend-transport";
import { RemoteDeadlineExceeded, withRemoteDeadline } from "./remote-deadline";
import { defaultRemoteDeadlines, remoteOperationBypassesQueue, remoteOperationClass, remoteOperationDeadlineClass, remoteOperationHadFalseMutationMarker, type RemoteDeadlines } from "./remote-operation-policy";
import { confirmRemoteHostCreationRecovery, setRemoteHostUnknownOutcome, setRemoteOutcomeReconciler, setRemoteUnknownOutcome } from "./remote-outcome-state";
import { clearRemoteOutcomeMarker, clearRestoredRemoteOutcomeMarker, remoteOutcomeMarkerIsCurrent, loadRemoteOutcomeMarkers, persistRemoteOutcomeMarker, remoteOutcomeHostOrigin, safeRemoteCanonicalProjectId, type RemoteOutcomeMarker } from "./remote-outcome-storage";
import { cacheMediaTicketsForResult, remoteResourceUrl, resetRemoteResourceSession } from "./remote-resource-cache";
import { remoteCsrfToken, setRemoteCsrfToken } from "./remote-credentials";
import { ExpiredServerRequestOutcome, RemoteOutcomeCapacityError, restoreServerOutcomes, ServerOutcomeClient, type ServerRequestOutcome } from "./remote-server-outcomes";
import { validateCanonicalReconciliation } from "./remote-outcome-reconciliation";
import { type Fetcher, type RpcResponse, type RemoteEvent, rpcErrorCodes, randomId, stringField, projectIdFor, expectedRevisionFor, stringFieldFromUnknown, projectRevisionFromResult, canonicalProjectIdFromResult, remoteArtifactUrl } from "./remote-transport-fields";
import {
  remoteEditorLeaseToken,
  remoteProjectAccess,
  setRemoteProjectAccess,
  setRemoteProjectRevision,
  setRemoteTakeoverHandler,
  setRemoteLeaseRefreshHandler,
  resetRemoteProjectAccess,
} from "./remote-project-access";

interface RemoteTransportOptions {
  readonly csrfToken: string;
  readonly hostLabel?: string;
  readonly fetcher?: Fetcher;
  readonly webSocketFactory?: (url: string, protocols: string[]) => WebSocket;
  readonly snapshotRequiredHandler?: () => void;
  readonly deadlines?: Partial<RemoteDeadlines>;
  readonly timingHandler?: (timing: RemoteRequestTiming) => void;
  readonly maximumPendingPerProject?: number;
  readonly outcomeProtocol?: 1 | undefined;
}

export interface RemoteRequestTiming {
  readonly operation: string;
  readonly requestId: string;
  readonly phase: RemoteRequestPhase;
  readonly durationMs: number;
  readonly outcome: "success" | "failed";
}

let currentSessionGeneration = 0;
const maximumCanonicalProjectIds = 32;
const remoteWorkflowBuilders = new Set([
  "build_temporal_job_summary", "build_temporal_transcribe_media_start_request",
  "build_temporal_generate_media_start_request", "build_temporal_start_result_action",
  "build_temporal_generate_media_failure_actions", "build_temporal_export_media_start_request",
  "build_temporal_export_project_bundle_start_request", "build_temporal_export_nle_xml_start_request",
  "build_temporal_codex_edit_start_request",
]);
const unconfirmedCreationMessage = "Project creation could not be confirmed. Refresh host projects and open the project from My Projects if it appears. New project creation stays paused while this request is unconfirmed.";
interface PendingOutcome {
  readonly envelope?: string | undefined;
  readonly error: RemoteOperationError;
  readonly marker: RemoteOutcomeMarker;
  readonly serverOnly: boolean;
}

export class RemoteTransport implements BackendTransport {
  readonly kind = "remote" as const;
  private readonly sessionGeneration = ++currentSessionGeneration;
  private readonly fetcher: Fetcher;
  private readonly listeners = new Map<string, Set<(payload: unknown) => void>>();
  private socket: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;
  private lastSequence = 0;
  private stopped = false;
  private readonly projectQueues = new Map<string, Promise<void>>();
  private readonly deadlines: RemoteDeadlines;
  private readonly unknownOutcomes = new Map<string, PendingOutcome>();
  private unknownHostOutcome?: PendingOutcome | undefined;
  private hostOrigin = "";
  private recoveryStorageError?: string;
  private readonly activeMutations = new Set<string>();
  private mutationGeneration = 0;
  private readonly canonicalProjectIds = new Map<string, string>();
  private readonly queueDepth = new Map<string, number>();
  private readonly leaseRequests = new Map<string, Promise<void>>();
  private readonly serverOutcomes?: ServerOutcomeClient;
  private serverRecovery?: Promise<void> | undefined;
  private serverRecoveryLoaded = false;

  constructor(private readonly options: RemoteTransportOptions) {
    this.fetcher = options.fetcher ?? globalThis.fetch.bind(globalThis);
    this.deadlines = { ...defaultRemoteDeadlines, ...options.deadlines };
    if (options.outcomeProtocol === 1) this.serverOutcomes = new ServerOutcomeClient(this.fetcher, () => this.csrfToken(), () => this.isCurrentSession(), this.deadlines.read);
    setRemoteCsrfToken(options.csrfToken);
    resetRemoteResourceSession();
    resetRemoteProjectAccess();
    setRemoteTakeoverHandler((projectId) => this.acquireProjectLease(projectId, true));
    setRemoteLeaseRefreshHandler(async (projectId) => {
      try { await this.acquireProjectLease(projectId, false); }
      catch (error) { this.recordLeaseFailure(projectId, error); throw error; }
    });
    setRemoteOutcomeReconciler((projectId, expectedProjectId) => this.reconcileProject(projectId, expectedProjectId));
    try {
      this.hostOrigin = remoteOutcomeHostOrigin();
      for (const marker of loadRemoteOutcomeMarkers(this.hostOrigin)) {
        if (remoteOperationHadFalseMutationMarker(marker.operation)) clearRemoteOutcomeMarker(marker.hostOrigin, marker.projectId, marker.requestId);
        else this.retainUnknown(marker);
      }
    } catch (error) {
      this.recoveryStorageError = error instanceof Error ? error.message : "Browser recovery storage is unavailable. Enable browser storage before editing. The request was not sent.";
    }
  }

  request<Result>(operation: string, input: BackendInput = {}): Promise<Result> {
    // Desktop builders stay internal. Remote equivalents resolve the opened catalog
    // identity and validate against the host's saved project before constructing data.
    if (remoteWorkflowBuilders.has(operation)) operation = `remote_${operation}`;
    else if (operation === "start_temporal_workflow") operation = "remote_start_temporal_workflow";
    let projectId = projectIdFor(input);
    if (projectId && !this.canonicalProjectIds.has(projectId)) {
      const opened = [...this.canonicalProjectIds].find(([, canonicalId]) => canonicalId === projectId);
      if (opened) projectId = opened[0];
    }
    const requestId = this.options.outcomeProtocol === 1 ? `browser-v2-${Math.floor(Date.now() / 1000)}-${randomId()}` : `browser-${randomId()}`;
    if (projectId && !this.unknownOutcomes.has(projectId) && this.unknownOutcomes.size >= 8 && remoteOperationClass(operation, input) !== "read" && !operation.startsWith("cancel_")) {
      return Promise.reject(new RemoteOperationError(operation, "busy", "Refresh unconfirmed projects before starting more work.", requestId, "queue", "not_sent"));
    }
    if (!projectId || remoteOperationBypassesQueue(operation)) {
      return this.performRequest<Result>(operation, input, projectId, requestId);
    }
    const previous = this.projectQueues.get(projectId) ?? Promise.resolve();
    const depth = this.queueDepth.get(projectId) ?? 0;
    if (depth >= (this.options.maximumPendingPerProject ?? 64)) {
      return Promise.reject(new RemoteOperationError(operation, "busy", "Too many requests are waiting. Try again after they settle.", requestId, "queue", "not_sent", 250));
    }
    this.queueDepth.set(projectId, depth + 1);
    const started = Date.now();
    let abandoned = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const queued = previous.catch(() => undefined).then(() => {
      if (timer !== undefined) clearTimeout(timer);
      if (abandoned) throw new RemoteOperationError(operation, "deadline_exceeded", "The request waited too long. It was not sent.", requestId, "queue", "not_sent");
      this.recordTiming(operation, requestId, "queue", started, "success");
      return this.performRequest<Result>(operation, input, projectId, requestId);
    });
    const admissionDeadline = new Promise<never>((_resolve, reject) => {
      timer = setTimeout(() => {
        abandoned = true;
        this.recordTiming(operation, requestId, "queue", started, "failed");
        reject(new RemoteOperationError(operation, "deadline_exceeded", "The request waited too long. It was not sent.", requestId, "queue", "not_sent"));
      }, this.deadlines.queue);
    });
    const result = Promise.race([queued, admissionDeadline]);
    // A timed-out queued caller does not release the preceding writer's FIFO reservation.
    const settled = queued.then(() => undefined, () => undefined);
    this.projectQueues.set(projectId, settled);
    void settled.finally(() => {
      const remaining = (this.queueDepth.get(projectId) ?? 1) - 1;
      if (remaining > 0) this.queueDepth.set(projectId, remaining);
      else this.queueDepth.delete(projectId);
      if (this.projectQueues.get(projectId) === settled) this.projectQueues.delete(projectId);
    });
    return result;
  }

  pendingOutcome(projectId?: string) {
    return projectId ? this.unknownOutcomes.get(projectId) : this.unknownHostOutcome;
  }

  /** Read canonical state; never resend a mutation whose response was lost. */
  async reconcileProject<Result>(projectId: string, expectedProjectId?: string): Promise<Result> {
    if (this.serverOutcomes) {
      try { await this.synchronizeServerOutcomes(); }
      catch (error) { if (!(error instanceof RemoteOutcomeCapacityError) || !this.unknownOutcomes.has(projectId)) throw error; }
    }
    if (this.activeMutations.has(projectId)) throw new Error("The last operation is still running. Wait for it to settle before refreshing.");
    const pending = this.unknownOutcomes.get(projectId);
    const mutationGeneration = this.mutationGeneration;
    const lookup = pending && this.serverOutcomes ? await this.serverOutcomes.get(pending.marker.requestId) : undefined;
    if (lookup instanceof ExpiredServerRequestOutcome && expectedProjectId === undefined) throw new Error("Open the project before refreshing to verify its identity.");
    const receipt = lookup instanceof ExpiredServerRequestOutcome ? undefined : lookup;
    if (receipt && (receipt.projectId !== projectId || receipt.operation !== pending?.marker.operation || receipt.status === "pending")) throw pending?.error ?? new Error("The operation is still running.");
    if (pending && pending.envelope === undefined && pending.marker.canonicalProjectId === undefined && expectedProjectId === undefined) {
      throw new Error("Open the project before refreshing to verify its identity.");
    }
    const requireSameOutcome = () => {
      if (this.mutationGeneration !== mutationGeneration || this.unknownOutcomes.get(projectId) !== pending || pending && !remoteOutcomeMarkerIsCurrent(pending.marker, pending.serverOnly)) {
        throw new Error("The unconfirmed operation changed during refresh. Refresh the project again.");
      }
    };
    const result = await this.performRequest<Result>(
      "read_project_snapshot_from_split_project_folder",
      { projectDir: projectId },
      projectId,
      `browser-${randomId()}`,
      (snapshot) => {
        requireSameOutcome();
        validateCanonicalReconciliation(snapshot, receipt, pending?.marker, expectedProjectId);
      },
    );
    this.requireCurrentSession("reconcile_project", `reconcile-${randomId()}`);
    requireSameOutcome();
    if (lookup) {
      await this.serverOutcomes!.acknowledge(lookup.requestId);
      this.requireCurrentSession("reconcile_project", `reconcile-${randomId()}`);
      requireSameOutcome();
    }
    if (pending && !clearRestoredRemoteOutcomeMarker(pending.marker, pending.serverOnly)) {
      throw new Error("The unconfirmed operation changed during refresh. Refresh the project again.");
    }
    this.unknownOutcomes.delete(projectId);
    setRemoteUnknownOutcome(projectId, null);
    return result;
  }

  private recordTiming(operation: string, requestId: string, phase: RemoteRequestPhase, started: number, outcome: RemoteRequestTiming["outcome"]): void {
    // Instrumentation must never change the result of a canonical operation.
    try { this.options.timingHandler?.({ operation, requestId, phase, durationMs: Date.now() - started, outcome }); } catch { /* observational only */ }
  }

  private async performRequest<Result>(
    operation: string,
    input: BackendInput,
    projectId: string | undefined,
    requestId: string,
    validateResult?: (result: Result) => void,
  ): Promise<Result> {
    this.requireCurrentSession(operation, requestId);
    const operationClass = remoteOperationClass(operation, input);
    const mutation = operationClass !== "read";
    if (this.serverOutcomes && operationClass !== "cancellation" && (mutation || operation === "remote_list_projects" || !this.serverRecoveryLoaded)) {
      try { await this.synchronizeServerOutcomes(); }
      catch (error) { if (mutation) throw new RemoteOperationError(operation, "network_unavailable", error instanceof Error ? error.message : "Host recovery is unavailable. Refresh before editing.", requestId, "rpc", "not_sent"); }
      this.requireCurrentSession(operation, requestId);
    }
    const pending = projectId ? this.unknownOutcomes.get(projectId) : this.unknownHostOutcome;
    if (pending && mutation && operationClass !== "cancellation") throw pending.error;
    if (
      projectId &&
      operation !== "load_split_project_from_folder" &&
      !remoteOperationBypassesQueue(operation)
    ) {
      const started = Date.now();
      try { await this.acquireProjectLease(projectId, false); }
      catch (error) {
        this.recordTiming(operation, requestId, "lease", started, "failed");
        if (error instanceof RemoteOperationError) throw new RemoteOperationError(operation, error.code, error.message, requestId, "lease", "not_sent", error.retryAfterMs);
        throw error;
      }
      this.recordTiming(operation, requestId, "lease", started, "success");
      this.requireCurrentSession(operation, requestId);
      if (mutation && !remoteEditorLeaseToken(projectId) && !stringField(input, "editorLeaseToken")) throw new RemoteOperationError(operation, "conflict", "Another device has editing access. Take over editing before making changes.", requestId, "lease", "not_sent");
    }
    const envelope = JSON.stringify({
      requestId, operation, projectId, expectedRevision: expectedRevisionFor(input, projectId),
      editorLeaseToken: stringField(input, "editorLeaseToken") ?? (projectId ? remoteEditorLeaseToken(projectId) : undefined),
      payload: input,
    });
    // This marker precedes fetch, so interruption while awaiting a response survives reload.
    // The exact envelope stays in memory only for the active session.
    const guardedMutation = mutation && operationClass !== "cancellation";
    const marker: RemoteOutcomeMarker = {
      hostOrigin: this.hostOrigin, requestId, operation, projectId,
      canonicalProjectId: safeRemoteCanonicalProjectId(stringFieldFromUnknown(input.project, "id") ?? (projectId ? this.canonicalProjectIds.get(projectId) : undefined)),
      phase: "rpc", outcome: "pending",
    };
    if (guardedMutation) {
      try {
        if (this.recoveryStorageError) throw new Error(this.recoveryStorageError);
        persistRemoteOutcomeMarker(marker);
      } catch (error) {
        throw new RemoteOperationError(operation, "network_unavailable", error instanceof Error ? error.message : "Browser recovery storage is unavailable. The request was not sent.", requestId, "rpc", "not_sent");
      }
      this.activeMutations.add(projectId ?? "");
      this.mutationGeneration += 1;
    }
    const started = Date.now();
    let body: RpcResponse<Result>;
    let response: Response;
    try {
      ({ response, body } = await withRemoteDeadline(this.deadlines[remoteOperationDeadlineClass(operation, input)], async (signal) => {
        const response = await this.fetcher("/api/v1/rpc", {
          method: "POST",
          credentials: "same-origin",
          headers: {
            accept: "application/json",
            "content-type": "application/json",
            "x-csrf-token": this.csrfToken(),
            ...(this.options.outcomeProtocol === 1 ? { "x-video-creater-outcome-protocol": "1" } : {}),
          },
          body: envelope,
          signal,
        });
        // The host rejects authentication before parsing/dispatching an RPC envelope.
        if (response.status === 401 || response.status === 403) {
          return { response, body: { requestId, ok: false, error: { code: response.status === 401 ? "unauthorized" as const : "forbidden" as const, message: response.status === 401 ? "The host session expired. Sign in again." : "Request authentication failed. Reconnect to the host." } } };
        }
        const body: unknown = await response.json();
        if (typeof body !== "object" || body === null || !("ok" in body) || typeof body.ok !== "boolean" || !("requestId" in body) || typeof body.requestId !== "string") throw new Error("Invalid remote response");
        return { response, body: body as RpcResponse<Result> };
      }));
    } catch (cause) {
      this.recordTiming(operation, requestId, "rpc", started, "failed");
      const error = new RemoteOperationError(operation, mutation ? "outcome_unknown" : cause instanceof RemoteDeadlineExceeded ? "deadline_exceeded" : "network_unavailable",
        mutation ? operation === "remote_create_project"
          ? unconfirmedCreationMessage
          : "The host response was lost. This operation may have completed. Refresh the project before editing again." : "The host did not respond. Try again.",
        requestId, "rpc", mutation ? "unknown" : "unavailable");
      if (guardedMutation) this.markUnknown(marker, envelope, error);
      throw error;
    }
    if (body.requestId !== requestId) {
      this.recordTiming(operation, requestId, "rpc", started, "failed");
      const error = new RemoteOperationError(operation, mutation ? "outcome_unknown" : "invalid_request", "Remote response did not match the request", requestId, "rpc", mutation ? "unknown" : "rejected");
      if (guardedMutation) this.markUnknown(marker, envelope, error);
      throw error;
    }
    if (!response.ok || !body.ok) {
      this.recordTiming(operation, requestId, "rpc", started, "failed");
      if (this.isCurrentSession() && projectId && ["conflict", "unauthorized", "forbidden"].includes(body.error?.code ?? "")) {
        setRemoteProjectAccess(projectId, { mode: "unknown" });
        if (body.error?.code === "unauthorized") resetRemoteResourceSession();
      }
      const code = body.error?.code && rpcErrorCodes.has(body.error.code) ? body.error.code : "internal";
      const retryAfterMs = Number.isSafeInteger(body.error?.retryAfterMs) && (body.error?.retryAfterMs ?? -1) >= 0 ? body.error?.retryAfterMs : undefined;
      if (code === "outcome_unknown" || code === "outcome_expired") {
        const error = new RemoteOperationError(operation, code, body.error?.message ?? "The original operation remains unconfirmed.", requestId, "rpc", "unknown", retryAfterMs);
        if (guardedMutation) this.markUnknown(marker, envelope, error);
        throw error;
      }
      if (guardedMutation) this.finishKnownMutation(marker);
      else if (mutation) this.serverOutcomes?.bestEffortAcknowledge(requestId);
      throw new RemoteOperationError(operation, code, body.error?.message ?? "Remote operation failed", requestId, "rpc", "rejected", retryAfterMs);
    }
    this.recordTiming(operation, requestId, "rpc", started, "success");
    if (guardedMutation) this.finishKnownMutation(marker);
    else if (mutation) this.serverOutcomes?.bestEffortAcknowledge(requestId);
    const result = body.result as Result;
    // Replacing a connection clears its credentials. Late work cannot restore that session.
    if (!this.isCurrentSession()) return result;
    // Reconciliation validates identity before a response can install revisions or media.
    validateResult?.(result);
    if (projectId) {
      const canonicalId = canonicalProjectIdFromResult(result);
      if (canonicalId !== undefined) {
        // Historical custom IDs can route in memory; recovery markers sanitize them before storage.
        this.canonicalProjectIds.set(projectId, canonicalId);
        if (this.canonicalProjectIds.size > maximumCanonicalProjectIds) this.canonicalProjectIds.delete(this.canonicalProjectIds.keys().next().value!);
      }
      const revision = projectRevisionFromResult(result);
      if (revision !== undefined) setRemoteProjectRevision(projectId, revision);
      // Media availability is recoverable and cannot change a committed RPC outcome.
      this.prepareMedia(projectId, result, operation, requestId);
    }
    if (operation === "load_split_project_from_folder" && projectId) {
      void this.acquireProjectLease(projectId, false).catch((error: unknown) => this.recordLeaseFailure(projectId, error));
    }
    if (operation === "remote_create_project") {
      const createdProjectId = stringFieldFromUnknown(result, "catalogProjectId");
      if (createdProjectId) {
        const revision = projectRevisionFromResult(result);
        if (revision !== undefined) setRemoteProjectRevision(createdProjectId, revision);
        this.prepareMedia(createdProjectId, result, operation, requestId);
        void this.acquireProjectLease(createdProjectId, false).catch((error: unknown) => this.recordLeaseFailure(createdProjectId, error));
      }
    }
    return result;
  }

  private retainUnknown(marker: RemoteOutcomeMarker, envelope?: string, error?: RemoteOperationError, serverOnly = false): void {
    const failure = error ?? new RemoteOperationError(marker.operation, "outcome_unknown", marker.operation === "remote_create_project"
      ? unconfirmedCreationMessage
      : "The last operation may have completed. Refresh the project before editing again.", marker.requestId, "rpc", "unknown");
    const pending = { marker, envelope, error: failure, serverOnly };
    if (marker.projectId) {
      this.unknownOutcomes.set(marker.projectId, pending);
      setRemoteUnknownOutcome(marker.projectId, failure);
    } else {
      this.unknownHostOutcome = pending;
      setRemoteHostUnknownOutcome(this.options.hostLabel, failure);
    }
  }

  private markUnknown(marker: RemoteOutcomeMarker, envelope: string, error: RemoteOperationError): void {
    this.activeMutations.delete(marker.projectId ?? "");
    if (!this.isCurrentSession()) return;
    const unknown = { ...marker, outcome: "unknown" as const };
    // The pre-dispatch marker remains safe if storage becomes unavailable now.
    try { persistRemoteOutcomeMarker(unknown); } catch { /* retain original recovery marker */ }
    this.retainUnknown(unknown, envelope, error);
  }

  private finishKnownMutation(marker: RemoteOutcomeMarker): void {
    this.activeMutations.delete(marker.projectId ?? "");
    if (!this.isCurrentSession()) return;
    try { clearRemoteOutcomeMarker(this.hostOrigin, marker.projectId, marker.requestId); }
    catch (error) { this.recoveryStorageError = error instanceof Error ? error.message : "Browser recovery storage is unavailable. The request was not sent."; }
    this.serverOutcomes?.bestEffortAcknowledge(marker.requestId);
  }

  private async synchronizeServerOutcomes(): Promise<void> {
    if (!this.serverOutcomes) return;
    if (this.serverRecovery) return this.serverRecovery;
    const generation = this.mutationGeneration;
    const synchronize = async () => {
      const receipts = await this.serverOutcomes!.list();
      if (!this.isCurrentSession() || generation !== this.mutationGeneration) throw new Error("The operation or host connection changed during recovery. Refresh again.");
      restoreServerOutcomes(receipts, (receipt) => this.clearRecoveredCreation(receipt), (receipt) => {
        const existing = receipt.projectId ? this.unknownOutcomes.get(receipt.projectId) : this.unknownHostOutcome;
        if (existing || this.activeMutations.has(receipt.projectId ?? "")) return;
        if (this.unknownOutcomes.size + Number(this.unknownHostOutcome !== undefined) >= 32) throw new RemoteOutcomeCapacityError();
        const marker: RemoteOutcomeMarker = { hostOrigin: this.hostOrigin, requestId: receipt.requestId, operation: receipt.operation, projectId: receipt.projectId, canonicalProjectId: safeRemoteCanonicalProjectId(receipt.canonicalProjectId), phase: "rpc", outcome: "unknown" };
        let serverOnly = false;
        try { persistRemoteOutcomeMarker(marker); } catch { serverOnly = true; }
        this.retainUnknown(marker, undefined, undefined, serverOnly);
      });
      this.serverRecoveryLoaded = true;
    };
    this.serverRecovery = synchronize();
    try { await this.serverRecovery; } finally { this.serverRecovery = undefined; }
  }

  private clearRecoveredCreation(receipt: ServerRequestOutcome): void {
    if (this.unknownHostOutcome?.marker.requestId === receipt.requestId && this.unknownHostOutcome.marker.operation === receipt.operation && clearRestoredRemoteOutcomeMarker(this.unknownHostOutcome.marker, this.unknownHostOutcome.serverOnly)) {
      this.unknownHostOutcome = undefined;
      setRemoteHostUnknownOutcome(this.options.hostLabel, null);
      confirmRemoteHostCreationRecovery(this.options.hostLabel);
    }
    this.serverOutcomes!.bestEffortAcknowledge(receipt.requestId);
  }

  private prepareMedia(projectId: string, result: unknown, operation: string, requestId: string): void {
    const ticketFetcher: Fetcher = async (url, init) => {
      const started = Date.now();
      try {
        // Include body parsing in the deadline by buffering the response inside the race.
        const response = await withRemoteDeadline(this.deadlines.ticket, async (signal) => {
          const response = await this.fetcher(url, { ...init, signal });
          const body = await response.text();
          return new Response(body, { status: response.status, headers: response.headers });
        });
        this.recordTiming(operation, requestId, "ticket", started, response.ok ? "success" : "failed");
        return response;
      } catch (cause) {
        this.recordTiming(operation, requestId, "ticket", started, "failed");
        throw new Error(cause instanceof RemoteDeadlineExceeded ? "Project media did not respond. Retry to reconnect." : "Project media could not be loaded. Retry to reconnect.");
      }
    };
    void cacheMediaTicketsForResult(projectId, result, this.csrfToken(), ticketFetcher);
  }

  private recordLeaseFailure(projectId: string, error: unknown): void {
    if (!this.isCurrentSession()) return;
    setRemoteProjectAccess(projectId, { mode: "unavailable", message: error instanceof Error ? error.message : "Editing access could not be acquired." });
  }

  private isCurrentSession(): boolean { return this.sessionGeneration === currentSessionGeneration; }

  private requireCurrentSession(operation: string, requestId: string): void {
    if (!this.isCurrentSession()) throw new RemoteOperationError(operation, "unauthorized", "This host connection was replaced. Use the current connection.", requestId, "queue", "not_sent");
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
    return remoteArtifactUrl(projectId, artifactId, this.fetcher, this.csrfToken(), this.deadlines.ticket);
  }

  private async acquireProjectLease(projectId: string, takeover: boolean): Promise<void> {
    const access = remoteProjectAccess(projectId);
    if (!takeover && access.mode === "editing" && remoteEditorLeaseToken(projectId) && access.expiresAt * 1000 > Date.now() + 5000) return;
    const pending = this.leaseRequests.get(projectId);
    if (pending) {
      if (!takeover) return pending;
      await pending.catch(() => undefined);
    }
    const request = this.fetchProjectLease(projectId, takeover);
    this.leaseRequests.set(projectId, request);
    try { await request; }
    finally { if (this.leaseRequests.get(projectId) === request) this.leaseRequests.delete(projectId); }
  }

  private async fetchProjectLease(projectId: string, takeover: boolean): Promise<void> {
    const started = Date.now();
    const requestId = `lease-${randomId()}`;
    let response: Response;
    let body: { readonly mode?: "editor" | "readOnly"; readonly editorLeaseToken?: string; readonly expiresAt?: number; readonly editorDisplayName?: string; readonly message?: string };
    try {
      ({ response, body } = await withRemoteDeadline(this.deadlines.lease, async (signal) => {
        const response = await this.fetcher(`/api/v1/projects/${encodeURIComponent(projectId)}/lease`, {
          method: "POST",
          credentials: "same-origin",
          headers: {
            accept: "application/json",
            "content-type": "application/json",
            "x-csrf-token": this.csrfToken(),
          },
          body: JSON.stringify({ takeover }),
          signal,
        });
        return { response, body: await response.json() as typeof body };
      }));
    } catch (cause) {
      this.recordTiming("acquire_editor_lease", requestId, "lease", started, "failed");
      throw new RemoteOperationError("acquire_editor_lease", cause instanceof RemoteDeadlineExceeded ? "deadline_exceeded" : "network_unavailable", "Editing access could not be acquired. Try again.", requestId, "lease", "not_sent");
    }
    if (!this.isCurrentSession()) {
      this.recordTiming("acquire_editor_lease", requestId, "lease", started, response.ok ? "success" : "failed");
      return;
    }
    if (response.status === 409 && body.mode === "readOnly") {
      setRemoteProjectAccess(projectId, {
        mode: "readOnly",
        editorDisplayName: body.editorDisplayName ?? "Another device",
      });
      this.recordTiming("acquire_editor_lease", requestId, "lease", started, "failed");
      return;
    }
    if (!response.ok || body.mode !== "editor" || !body.editorLeaseToken || typeof body.expiresAt !== "number") {
      this.recordTiming("acquire_editor_lease", requestId, "lease", started, "failed");
      throw new RemoteOperationError("acquire_editor_lease", response.status === 401 ? "unauthorized" : response.status === 403 ? "forbidden" : response.status === 409 ? "conflict" : "internal", body.message ?? "Editing access could not be acquired.", requestId, "lease", "not_sent");
    }
    setRemoteProjectAccess(projectId, {
      mode: "editing",
      token: body.editorLeaseToken,
      expiresAt: body.expiresAt,
    });
    this.recordTiming("acquire_editor_lease", requestId, "lease", started, "success");
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
