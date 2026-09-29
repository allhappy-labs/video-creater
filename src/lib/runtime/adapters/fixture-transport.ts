import {
  FixtureOperationUnsupportedError,
  type BackendInput,
  type BackendTransport,
  type BackendUnlisten,
} from "../backend-transport";

export type FixtureOperationHandler = (
  input: BackendInput,
) => unknown | Promise<unknown>;

type FixtureEventHandler = (payload: unknown) => void;

export class FixtureTransport implements BackendTransport {
  readonly kind = "fixture" as const;
  private readonly eventHandlers = new Map<string, Set<FixtureEventHandler>>();

  constructor(
    private readonly operations: ReadonlyMap<string, FixtureOperationHandler>,
  ) {}

  async request<Result>(
    operation: string,
    input: BackendInput = {},
  ): Promise<Result> {
    const handler = this.operations.get(operation);
    if (!handler) {
      throw new FixtureOperationUnsupportedError(operation);
    }
    const result = await handler(structuredClone(input));
    return structuredClone(result) as Result;
  }

  async listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten> {
    const handlers = this.eventHandlers.get(event) ?? new Set<FixtureEventHandler>();
    const fixtureHandler: FixtureEventHandler = (payload) => {
      handler(structuredClone(payload) as Payload);
    };
    handlers.add(fixtureHandler);
    this.eventHandlers.set(event, handlers);
    return () => {
      handlers.delete(fixtureHandler);
      if (handlers.size === 0) {
        this.eventHandlers.delete(event);
      }
    };
  }

  mediaUrl(path: string): string {
    const segments = path
      .replace(/\\/g, "/")
      .split("/")
      .filter(Boolean)
      .map(encodeURIComponent);
    return `/__editor-fixture-media/${segments.join("/")}`;
  }

  emitForVisualQa<Payload>(event: string, payload: Payload): void {
    for (const handler of this.eventHandlers.get(event) ?? []) {
      handler(payload);
    }
  }
}
