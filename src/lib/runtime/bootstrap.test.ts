import { describe, expect, it } from "vitest";

import { FixtureTransport } from "./adapters/fixture-transport";
import { selectRuntime } from "./bootstrap";

describe("selectRuntime", () => {
  it("gives an explicit fixture runtime priority over desktop detection", () => {
    const fixtureTransport = new FixtureTransport(new Map());

    const runtime = selectRuntime({
      fixtureTransport,
      tauriAvailable: true,
    });

    expect(runtime).toMatchObject({
      mode: "fixture",
      connection: {
        status: "connected",
        transport: fixtureTransport,
      },
    });
  });

  it("selects the connected desktop runtime when Tauri is available", () => {
    const runtime = selectRuntime({
      fixtureTransport: null,
      tauriAvailable: true,
    });

    expect(runtime.mode).toBe("desktop");
    expect(runtime.connection).toMatchObject({
      status: "connected",
      transport: { kind: "tauri" },
    });
  });

  it("selects a disconnected browser without inferring fixture mode", () => {
    expect(
      selectRuntime({
        fixtureTransport: null,
        tauriAvailable: false,
      }),
    ).toEqual({
      mode: "browser",
      connection: { status: "disconnected" },
    });
  });
});
