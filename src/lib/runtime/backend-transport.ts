type BackendTransportKind = "tauri" | "fixture" | "remote";
export type BackendInput = Record<string, unknown>;
export type BackendUnlisten = () => void;
export type RemoteErrorCode = "invalid_request" | "unauthorized" | "forbidden" | "not_found" | "conflict" | "rate_limited" | "busy" | "internal" | "deadline_exceeded" | "outcome_unknown" | "outcome_expired" | "network_unavailable";
export type RemoteRequestPhase = "queue" | "lease" | "rpc" | "ticket";
export type RemoteRequestOutcome = "rejected" | "not_sent" | "unknown" | "unavailable";

/** A host rejection and a lost mutation response have different recovery rules. */
export class RemoteOperationError extends Error {
  constructor(
    readonly operation: string,
    readonly code: RemoteErrorCode,
    message: string,
    readonly requestId: string,
    readonly phase: RemoteRequestPhase,
    readonly outcome: RemoteRequestOutcome,
    readonly retryAfterMs?: number,
  ) {
    super(message);
    this.name = "RemoteOperationError";
  }
}

export function isUnknownRemoteOutcome(error: unknown): error is RemoteOperationError {
  return error instanceof RemoteOperationError && error.outcome === "unknown";
}

export interface BackendTransport {
  readonly kind: BackendTransportKind;

  request<Result>(
    operation: string,
    input?: BackendInput,
  ): Promise<Result>;

  listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten>;

  mediaUrl(path: string): string;

  artifactUrl?(projectId: string, artifactId: string): Promise<string>;
}

export class BackendUnavailableError extends Error {
  readonly code = "backend_unavailable";

  constructor() {
    super("A backend connection is required for this operation");
    this.name = "BackendUnavailableError";
  }
}

export function isBackendUnavailableError(
  error: unknown,
): error is BackendUnavailableError {
  return (
    error instanceof BackendUnavailableError ||
    (
      typeof error === "object" &&
      error !== null &&
      "code" in error &&
      error.code === "backend_unavailable"
    )
  );
}

export class FixtureOperationUnsupportedError extends Error {
  readonly code = "fixture_operation_unsupported";

  constructor(readonly operation: string) {
    super(`Fixture operation is not implemented: ${operation}`);
    this.name = "FixtureOperationUnsupportedError";
  }
}

export class BackendOperationError extends Error {
  readonly code = "backend_operation_failed";

  constructor(
    readonly operation: string,
    readonly cause: unknown,
  ) {
    super(
      cause instanceof Error
        ? `${operation}: ${cause.message}`
        : `${operation}: backend operation failed`,
    );
    this.name = "BackendOperationError";
  }
}
