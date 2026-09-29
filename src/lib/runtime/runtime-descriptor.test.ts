import { describe, expect, it } from "vitest";

import type { RuntimeMode } from "./runtime-descriptor";
import { createRuntimeDescriptor } from "./runtime-descriptor";
import type { BackendTransport } from "./backend-transport";

const transport: BackendTransport = {
  kind: "fixture",
  async request<Result>() {
    return undefined as Result;
  },
  async listen() {
    return () => undefined;
  },
  mediaUrl(path) {
    return `fixture-media:${path}`;
  },
};

describe("createRuntimeDescriptor", () => {
  it("creates an immutable disconnected browser descriptor", () => {
    const descriptor = createRuntimeDescriptor("browser", {
      status: "disconnected",
    });

    expect(descriptor).toEqual({
      mode: "browser",
      connection: { status: "disconnected" },
    });
    expect(Object.isFrozen(descriptor)).toBe(true);
    expect(Object.isFrozen(descriptor.connection)).toBe(true);
  });

  it.each(["desktop", "fixture"] as const)(
    "requires %s mode to have a connected transport",
    (mode) => {
      expect(() =>
        createRuntimeDescriptor(mode, { status: "disconnected" }),
      ).toThrow(`${mode} runtime requires a connected backend`);
    },
  );

  it("allows a browser descriptor to become connected in a future pairing flow", () => {
    expect(
      createRuntimeDescriptor("browser", {
        status: "connected",
        transport,
      }),
    ).toMatchObject({
      mode: "browser",
      connection: { status: "connected", transport },
    });
  });

  it("rejects unknown runtime modes at the runtime boundary", () => {
    expect(() =>
      createRuntimeDescriptor("embedded" as RuntimeMode, {
        status: "connected",
        transport,
      }),
    ).toThrow("Unsupported runtime mode: embedded");
  });
});
