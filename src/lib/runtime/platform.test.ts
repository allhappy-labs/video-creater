import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { backendRequest } from "./backend-client";
import {
  defaultHostPlatform,
  getHostPlatform,
  getPlatformInfo,
  hostPlatformCopy,
  installHostPlatform,
  loadHostPlatform,
  useHostPlatformCopy,
} from "./platform";

vi.mock("./backend-client", () => ({
  backendRequest: vi.fn(),
}));

const requestMock = vi.mocked(backendRequest);

describe("host platform", () => {
  afterEach(() => {
    requestMock.mockReset();
    installHostPlatform(defaultHostPlatform);
  });

  it("defaults to the macOS presentation used by fixture baselines", () => {
    expect(defaultHostPlatform).toBe("macos");
    expect(getHostPlatform()).toBe("macos");
  });

  it("reads the typed platform command and rejects unknown platforms", async () => {
    requestMock.mockResolvedValueOnce({ platform: "linux" });
    await expect(getPlatformInfo()).resolves.toEqual({ platform: "linux" });
    expect(requestMock).toHaveBeenCalledWith("get_platform_info");

    requestMock.mockResolvedValueOnce({ platform: "beos" });
    await expect(getPlatformInfo()).rejects.toThrow("unsupported platform");
  });

  it("installs the backend platform and keeps the default when it is unavailable", async () => {
    requestMock.mockRejectedValueOnce(new Error("unknown command"));
    await expect(loadHostPlatform()).resolves.toBe("macos");

    requestMock.mockResolvedValueOnce({ platform: "linux" });
    await expect(loadHostPlatform()).resolves.toBe("linux");
    expect(getHostPlatform()).toBe("linux");
  });

  it("keeps macOS copy byte-identical and names Linux desktop services", () => {
    const macos = hostPlatformCopy("macos");
    expect(macos.credentialSavedLabel).toBe("Saved in macOS Keychain");
    expect(macos.revealLabel("Global models")).toBe("Reveal Global models in Finder");
    expect(macos.primaryModifier).toBe("Cmd");
    expect(macos.transcriptionHelperReady).toBe("The on-device Core ML helper is ready.");

    const linux = hostPlatformCopy("linux");
    expect(linux.credentialSavedLabel).toBe("Saved in system keyring");
    expect(linux.credentialRemovalDetail).not.toMatch(/Mac|Keychain/);
    expect(linux.revealLabel("Global models")).toBe("Show Global models in file manager");
    expect(linux.revealFailure("Global models", "no handler")).toBe(
      "The file manager could not show Global models: no handler",
    );
    expect(linux.primaryModifier).toBe("Ctrl");
    expect(Object.values(linux).filter((value) => typeof value === "string").join(" "))
      .not.toMatch(/macOS|Keychain|Finder|Core ML|Cmd/);
  });

  it("re-renders subscribers when the platform is installed", () => {
    const { result } = renderHook(() => useHostPlatformCopy());
    expect(result.current.primaryModifier).toBe("Cmd");
    act(() => installHostPlatform("linux"));
    expect(result.current.primaryModifier).toBe("Ctrl");
  });
});
