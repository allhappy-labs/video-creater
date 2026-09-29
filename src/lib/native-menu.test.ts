import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  inactiveNativeMenuState,
  listenForNativeMenuCommands,
  nativeMenuEventName,
  parseNativeMenuRequest,
} from "./native-menu";

const { listenMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
}));

vi.mock("@/lib/runtime/backend-client", () => ({
  backendListen: listenMock,
}));

describe("native menu bridge", () => {
  beforeEach(() => {
    listenMock.mockReset();
  });

  it("parses only exhaustive sequence-stamped commands", () => {
    expect(parseNativeMenuRequest({ sequence: 7, command: "openSettings" })).toEqual({
      sequence: 7,
      command: "openSettings",
    });
    expect(parseNativeMenuRequest({ sequence: 0, command: "openSettings" })).toBeNull();
    expect(parseNativeMenuRequest({ sequence: 7.5, command: "openSettings" })).toBeNull();
    expect(parseNativeMenuRequest({ sequence: 7, command: "unknown" })).toBeNull();
    expect(parseNativeMenuRequest("openSettings")).toBeNull();
  });

  it("accepts the redesigned editor commands and rejects removed ones", () => {
    for (const command of [
      "openProjectSettings",
      "openAdvancedSettings",
      "openSystemHealth",
      "undo",
      "redo",
      "showTab:ai",
      "showTab:media",
      "showTab:audio",
      "showTab:text",
      "showTab:captions",
      "showTab:effects",
      "openShortcuts",
      "openConnectAgents",
      "openProjectGuidance",
      "sendFeedback",
    ]) {
      expect(parseNativeMenuRequest({ sequence: 8, command })).toEqual({
        sequence: 8,
        command,
      });
    }

    for (const command of [
      "checkUpdates",
      "saveProject",
      "saveProjectAs",
      "openMcp",
      "openSkills",
      "toggleMedia",
      "toggleInspector",
      "toggleCodex",
      "toggleMaximize",
      "layoutDefault",
      "layoutMedia",
      "layoutVertical",
      "openTour",
      "showTab:properties",
    ]) {
      expect(parseNativeMenuRequest({ sequence: 8, command })).toBeNull();
    }
  });

  it("reports nothing actionable outside the editor", () => {
    expect(inactiveNativeMenuState("settings")).toEqual({
      view: "settings",
      canImport: false,
      canExport: false,
      canUndo: false,
      canRedo: false,
      canSplit: false,
      canTrimStart: false,
      canTrimEnd: false,
      canDelete: false,
      canRippleDelete: false,
      canSelectForward: false,
    });
  });

  it("registers once and drops malformed event payloads", async () => {
    const unlisten = vi.fn();
    let handler: (payload: unknown) => void = () => {};
    listenMock.mockImplementation(
      async (_eventName: string, next: (payload: unknown) => void) => {
        handler = next;
        return unlisten;
      },
    );
    const onRequest = vi.fn();

    const registeredUnlisten = await listenForNativeMenuCommands(onRequest);
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledWith(nativeMenuEventName, expect.any(Function));

    handler({ sequence: 2, command: "undo" });
    handler({ sequence: "2", command: "undo" });
    handler({ sequence: 3, command: "openTour" });
    expect(onRequest).toHaveBeenCalledTimes(1);
    expect(onRequest).toHaveBeenCalledWith({ sequence: 2, command: "undo" });
    expect(registeredUnlisten).toBe(unlisten);
  });
});
