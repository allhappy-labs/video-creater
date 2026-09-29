import { afterEach, describe, expect, it, vi } from "vitest";

const {
  convertFileSrcMock,
  emitMock,
  invokeMock,
  listenMock,
} = vi.hoisted(() => ({
  convertFileSrcMock: vi.fn(),
  emitMock: vi.fn(),
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: convertFileSrcMock,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: emitMock,
  listen: listenMock,
}));

import { createTauriSettingsAcceptanceBridge } from "./tauri-settings-acceptance-bridge";
import { TauriTransport } from "./tauri-transport";

afterEach(() => {
  vi.clearAllMocks();
});

describe("TauriTransport", () => {
  it("uses the native request result and input without changing the domain contract", async () => {
    invokeMock.mockResolvedValue({ projectId: "project-1" });
    const transport = new TauriTransport();

    await expect(
      transport.request("load_project", { projectDir: "/project" }),
    ).resolves.toEqual({ projectId: "project-1" });
    expect(invokeMock).toHaveBeenCalledWith("load_project", {
      projectDir: "/project",
    });
  });

  it("unwraps native event envelopes before calling domain listeners", async () => {
    const unlisten = vi.fn();
    let nativeHandler: (event: { payload: unknown }) => void = () => {
      throw new Error("native listener was not installed");
    };
    listenMock.mockImplementation(async (_event, handler) => {
      nativeHandler = handler;
      return unlisten;
    });
    const domainHandler = vi.fn();
    const transport = new TauriTransport();

    const result = await transport.listen("project-changed", domainHandler);
    nativeHandler({ payload: { revision: 4 } });

    expect(domainHandler).toHaveBeenCalledWith({ revision: 4 });
    expect(result).toBe(unlisten);
  });

  it("converts local media paths with the native asset protocol", () => {
    convertFileSrcMock.mockReturnValue("http://asset.localhost/source.mp4");

    expect(new TauriTransport().mediaUrl("/project/source.mp4")).toBe(
      "http://asset.localhost/source.mp4",
    );
    expect(convertFileSrcMock).toHaveBeenCalledWith("/project/source.mp4");
  });

  it("streams Linux media from the injected loopback base instead of asset URLs", () => {
    const host = globalThis as { __VIDEO_CREATER_MEDIA_STREAM_BASE__?: unknown };
    host.__VIDEO_CREATER_MEDIA_STREAM_BASE__ = "http://127.0.0.1:43123/media/0123abcd";
    convertFileSrcMock.mockClear();
    try {
      expect(new TauriTransport().mediaUrl("/project/source clip.mp4")).toBe(
        "http://127.0.0.1:43123/media/0123abcd/%2Fproject%2Fsource%20clip.mp4",
      );
      expect(convertFileSrcMock).not.toHaveBeenCalled();
      host.__VIDEO_CREATER_MEDIA_STREAM_BASE__ = "http://example.com/media/0123abcd";
      convertFileSrcMock.mockReturnValue("asset://localhost/source.mp4");
      expect(new TauriTransport().mediaUrl("/project/source.mp4")).toBe(
        "asset://localhost/source.mp4",
      );
    } finally {
      delete host.__VIDEO_CREATER_MEDIA_STREAM_BASE__;
    }
  });

  it("leaves native request failures available for BackendClient wrapping", async () => {
    const cause = new Error("permission denied");
    invokeMock.mockRejectedValue(cause);

    await expect(new TauriTransport().request("load_project")).rejects.toBe(
      cause,
    );
  });
});

describe("createTauriSettingsAcceptanceBridge", () => {
  it("provides the explicit desktop-only acceptance bridge", async () => {
    invokeMock.mockResolvedValue({ enabled: true });
    emitMock.mockResolvedValue(undefined);
    const bridge = createTauriSettingsAcceptanceBridge();

    await expect(bridge.invoke("get_settings_acceptance_context")).resolves.toEqual({
      enabled: true,
    });
    await expect(
      bridge.emit("native-menu-command", { command: "openSettings" }),
    ).resolves.toBeUndefined();
    expect(bridge.document).toBe(document);
  });
});
