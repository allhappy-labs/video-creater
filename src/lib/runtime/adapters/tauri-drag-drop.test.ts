import { afterEach, describe, expect, it, vi } from "vitest";

const { getCurrentWebviewMock, onDragDropEventMock } = vi.hoisted(() => ({
  getCurrentWebviewMock: vi.fn(),
  onDragDropEventMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: getCurrentWebviewMock }));

import type { FileDropEvent } from "../file-drop";
import { listenForFileDrops } from "./tauri-drag-drop";

type NativeHandler = (event: { payload: unknown }) => void;

function installNativeWebview(unlisten = vi.fn()) {
  window.__TAURI_INTERNALS__ = {};
  let nativeHandler: NativeHandler = () => {
    throw new Error("native drag-drop listener was not installed");
  };
  onDragDropEventMock.mockImplementation(async (handler: NativeHandler) => {
    nativeHandler = handler;
    return unlisten;
  });
  getCurrentWebviewMock.mockReturnValue({ onDragDropEvent: onDragDropEventMock });
  return { emit: (payload: unknown) => nativeHandler({ payload }), unlisten };
}

afterEach(() => {
  vi.clearAllMocks();
  delete window.__TAURI_INTERNALS__;
});

describe("tauri webview file drops", () => {
  it("maps enter/over to over, leave to leave, and drop to its paths", async () => {
    const native = installNativeWebview();
    const events: FileDropEvent[] = [];
    listenForFileDrops((event) => events.push(event));
    await vi.waitFor(() => expect(onDragDropEventMock).toHaveBeenCalled());

    native.emit({ type: "enter", paths: ["/tmp/a.mp4"], position: { x: 1, y: 2 } });
    native.emit({ type: "over", position: { x: 3, y: 4 } });
    native.emit({ type: "leave" });
    native.emit({ type: "drop", paths: ["/tmp/a.mp4", "/tmp/b.png"], position: { x: 5, y: 6 } });

    expect(events).toEqual([
      { type: "over" },
      { type: "over" },
      { type: "leave" },
      { type: "drop", paths: ["/tmp/a.mp4", "/tmp/b.png"] },
    ]);
  });

  it("unlistens once the native listener resolves", async () => {
    const native = installNativeWebview();
    const stop = listenForFileDrops(vi.fn());
    await vi.waitFor(() => expect(onDragDropEventMock).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 0));

    stop();

    expect(native.unlisten).toHaveBeenCalledTimes(1);
  });

  it("unlistens a listener that resolves after it was stopped and drops late events", async () => {
    const native = installNativeWebview();
    const handler = vi.fn();
    const stop = listenForFileDrops(handler);
    stop();
    await vi.waitFor(() => expect(native.unlisten).toHaveBeenCalledTimes(1));

    native.emit({ type: "drop", paths: ["/tmp/late.mp4"], position: { x: 0, y: 0 } });

    expect(handler).not.toHaveBeenCalled();
  });

  it("is a no-op outside the desktop app", () => {
    const stop = listenForFileDrops(vi.fn());

    expect(getCurrentWebviewMock).not.toHaveBeenCalled();
    expect(() => stop()).not.toThrow();
  });

  it("ignores a native listener that fails to install", async () => {
    window.__TAURI_INTERNALS__ = {};
    getCurrentWebviewMock.mockReturnValue({
      onDragDropEvent: vi.fn().mockRejectedValue(new Error("no webview")),
    });

    const stop = listenForFileDrops(vi.fn());
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(() => stop()).not.toThrow();
  });
});
