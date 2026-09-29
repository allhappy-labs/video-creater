import { afterEach, describe, expect, it, vi } from "vitest";

import {
  fixtureRuntimeMarkerEnabled,
  getRuntimeMode,
  installRuntimeMode,
  subscribeRuntimeMode,
} from "./runtime-mode";

describe("runtime mode", () => {
  afterEach(() => {
    installRuntimeMode("browser");
    delete window.__EDITOR_FIXTURE_RUNTIME__;
    vi.unstubAllEnvs();
  });

  it("defaults to the disconnected browser runtime", () => {
    expect(getRuntimeMode()).toBe("browser");
  });

  it("reports the installed desktop runtime and notifies subscribers once per change", () => {
    const listener = vi.fn();
    const unsubscribe = subscribeRuntimeMode(listener);

    installRuntimeMode("desktop");
    installRuntimeMode("desktop");
    expect(getRuntimeMode()).toBe("desktop");
    expect(listener).toHaveBeenCalledTimes(1);

    unsubscribe();
    installRuntimeMode("browser");
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("reports fixture only in a development build with the marker enabled", () => {
    vi.stubEnv("DEV", true);
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");

    expect(fixtureRuntimeMarkerEnabled()).toBe(true);
    expect(getRuntimeMode()).toBe("fixture");
  });

  it("never reports fixture without the marker", () => {
    vi.stubEnv("DEV", true);
    installRuntimeMode("fixture");

    expect(fixtureRuntimeMarkerEnabled()).toBe(false);
    expect(getRuntimeMode()).toBe("browser");
  });

  it("never reports fixture outside a development build", () => {
    vi.stubEnv("DEV", false);
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");

    expect(fixtureRuntimeMarkerEnabled()).toBe(false);
    expect(getRuntimeMode()).toBe("browser");
  });
});
