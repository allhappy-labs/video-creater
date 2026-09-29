import { describe, expect, it, vi } from "vitest";

import {
  BackendOperationError,
  BackendUnavailableError,
  FixtureOperationUnsupportedError,
  isBackendUnavailableError,
  type BackendTransport,
} from "./backend-transport";
import { BackendClient } from "./backend-client";

function recordingTransport(): BackendTransport {
  return {
    kind: "fixture",
    async request<Result>() {
      return { ok: true } as Result;
    },
    async listen() {
      return () => undefined;
    },
    mediaUrl(path) {
      return `fixture-media:${path}`;
    },
  };
}

describe("BackendClient", () => {
  it("recognizes only typed backend-unavailable failures", () => {
    expect(isBackendUnavailableError(new BackendUnavailableError())).toBe(true);
    expect(isBackendUnavailableError({ code: "backend_unavailable" })).toBe(true);
    expect(
      isBackendUnavailableError(
        new TypeError("Cannot read properties of undefined (reading 'invoke')"),
      ),
    ).toBe(false);
  });

  it("fails every backend-dependent operation while disconnected", async () => {
    const client = new BackendClient();
    client.install({ status: "disconnected" });

    await expect(client.request("load_project")).rejects.toMatchObject({
      code: "backend_unavailable",
    });
    await expect(client.listen("project-changed", vi.fn())).rejects.toBeInstanceOf(
      BackendUnavailableError,
    );
    expect(() => client.mediaUrl("/project/source.mp4")).toThrow(
      BackendUnavailableError,
    );
  });

  it("delegates requests, events, and media URLs to one connected transport", async () => {
    const transport = recordingTransport();
    const client = new BackendClient();
    const eventHandler = vi.fn();
    client.install({ status: "connected", transport });

    await expect(client.request("load_project", { projectDir: "/project" })).resolves.toEqual({
      ok: true,
    });
    const unlisten = await client.listen("project-changed", eventHandler);
    expect(client.mediaUrl("/project/source.mp4")).toBe(
      "fixture-media:/project/source.mp4",
    );
    expect(typeof unlisten).toBe("function");
  });

  it("wraps transport failures with the failed operation and original cause", async () => {
    const cause = new Error("native detail");
    const transport = recordingTransport();
    transport.request = async () => {
      throw cause;
    };
    const client = new BackendClient();
    client.install({ status: "connected", transport });

    const failure = await client.request("load_project").catch((error: unknown) => error);

    expect(failure).toBeInstanceOf(BackendOperationError);
    expect(failure).toMatchObject({
      code: "backend_operation_failed",
      operation: "load_project",
      cause,
    });
  });

  it("preserves explicit fixture-contract failures", async () => {
    const transport = recordingTransport();
    transport.request = async () => {
      throw new FixtureOperationUnsupportedError("missing_fixture_operation");
    };
    const client = new BackendClient();
    client.install({ status: "connected", transport });

    await expect(client.request("missing_fixture_operation")).rejects.toMatchObject({
      code: "fixture_operation_unsupported",
      operation: "missing_fixture_operation",
    });
  });

  it("rejects a second connection installation", () => {
    const client = new BackendClient();
    client.install({ status: "disconnected" });

    expect(() =>
      client.install({ status: "connected", transport: recordingTransport() }),
    ).toThrow("Backend connection is already installed");
  });
});
