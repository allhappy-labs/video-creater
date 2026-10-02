import {
  BackendOperationError,
  BackendUnavailableError,
  FixtureOperationUnsupportedError,
  RemoteOperationError,
  type BackendInput,
  type BackendTransport,
  type BackendUnlisten,
} from "./backend-transport";

export type BackendConnection =
  | { readonly status: "disconnected" }
  | {
      readonly status: "connected";
      readonly transport: BackendTransport;
    };

function backendFailure(operation: string, error: unknown): Error {
  if (
    error instanceof BackendUnavailableError ||
    error instanceof FixtureOperationUnsupportedError ||
    error instanceof BackendOperationError ||
    error instanceof RemoteOperationError
  ) {
    return error;
  }
  return new BackendOperationError(operation, error);
}

export class BackendClient {
  private connection: BackendConnection | null = null;

  install(connection: BackendConnection): void {
    if (this.connection !== null) {
      throw new Error("Backend connection is already installed");
    }
    this.connection = connection;
  }

  async request<Result>(
    operation: string,
    input?: BackendInput,
  ): Promise<Result> {
    const transport = this.connectedTransport();
    try {
      return await transport.request<Result>(operation, input);
    } catch (error) {
      throw backendFailure(operation, error);
    }
  }

  async listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten> {
    const transport = this.connectedTransport();
    try {
      return await transport.listen(event, handler);
    } catch (error) {
      throw backendFailure(`listen:${event}`, error);
    }
  }

  mediaUrl(path: string): string {
    const transport = this.connectedTransport();
    try {
      return transport.mediaUrl(path);
    } catch (error) {
      throw backendFailure("mediaUrl", error);
    }
  }

  async artifactUrl(projectId: string, artifactId: string): Promise<string> {
    const transport = this.connectedTransport();
    if (!transport.artifactUrl) {
      throw new BackendUnavailableError();
    }
    try {
      return await transport.artifactUrl(projectId, artifactId);
    } catch (error) {
      throw backendFailure("artifactUrl", error);
    }
  }

  private connectedTransport(): BackendTransport {
    if (this.connection?.status !== "connected") {
      throw new BackendUnavailableError();
    }
    return this.connection.transport;
  }
}

export const backendClient = new BackendClient();

export function backendRequest<Result>(
  operation: string,
  input?: BackendInput,
): Promise<Result> {
  return backendClient.request(operation, input);
}

export function backendListen<Payload>(
  event: string,
  handler: (payload: Payload) => void,
): Promise<BackendUnlisten> {
  return backendClient.listen(event, handler);
}

export function backendMediaUrl(path: string): string {
  return backendClient.mediaUrl(path);
}

export function backendArtifactUrl(projectId: string, artifactId: string): Promise<string> {
  return backendClient.artifactUrl(projectId, artifactId);
}
