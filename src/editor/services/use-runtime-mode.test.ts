import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { installRuntimeMode } from "@/lib/runtime/runtime-mode";

import { useRuntimeMode } from "./use-runtime-mode";

describe("useRuntimeMode", () => {
  afterEach(() => {
    installRuntimeMode("browser");
    delete window.__EDITOR_FIXTURE_RUNTIME__;
    vi.unstubAllEnvs();
  });

  it("returns fixture when the marker is enabled in a development build", () => {
    vi.stubEnv("DEV", true);
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");

    const { result } = renderHook(() => useRuntimeMode());

    expect(result.current).toBe("fixture");
  });

  it("does not return fixture when the marker is absent", () => {
    vi.stubEnv("DEV", true);
    installRuntimeMode("fixture");

    const { result } = renderHook(() => useRuntimeMode());

    expect(result.current).toBe("browser");
  });

  it("does not return fixture in a production build", () => {
    vi.stubEnv("DEV", false);
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");

    const { result } = renderHook(() => useRuntimeMode());

    expect(result.current).toBe("browser");
  });

  it("follows runtime installation", () => {
    const { result } = renderHook(() => useRuntimeMode());
    expect(result.current).toBe("browser");

    act(() => installRuntimeMode("desktop"));

    expect(result.current).toBe("desktop");
  });
});
