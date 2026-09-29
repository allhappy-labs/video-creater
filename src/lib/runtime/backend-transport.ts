type BackendTransportKind = "tauri" | "fixture" | "remote";
export type BackendInput = Record<string, unknown>;
export type BackendUnlisten = () => void;

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
