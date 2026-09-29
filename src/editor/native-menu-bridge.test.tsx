import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { NativeMenuRequest, NativeMenuState } from "@/lib/native-menu";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => path,
}));

const { createEditorStore } = await import("./store/editor-store");
const { EditorStoreProvider } = await import("./store/editor-store-context");
const { useEditorShortcuts } = await import("./shell/use-editor-shortcuts");
const { editorNativeMenuState, runNativeEditorCommand, useNativeMenuBridge } = await import("./native-menu-bridge");

type Store = ReturnType<typeof createEditorStore>;

function createStore(projectDir = "/p") {
  const project = { ...fixtureProject(), schemaVersion: 2, contentRevision: 1 };
  return { store: createEditorStore({ projectDir, project }), project };
}

/** Selects the first video clip and parks the playhead inside it. */
function selectClipUnderPlayhead(store: Store) {
  const item = fixtureItem(store.getState().project, "video");
  store.getState().selectItems([item.id]);
  store.getState().seek(item.startSeconds + item.durationSeconds / 2);
  return item;
}

function commandContext(store: Store, overrides: { mobile?: boolean; focused?: Element | null } = {}) {
  const timeline = {
    splitAtPlayhead: vi.fn(async () => true),
    trimToPlayhead: vi.fn(async () => true),
    deleteSelection: vi.fn(async () => true),
    rippleDeleteSelection: vi.fn(async () => true),
    deleteGap: vi.fn(async () => true),
  };
  const media = { importMediaFiles: vi.fn(async () => ({ status: "cancelled" as const })) };
  return {
    timeline,
    media,
    context: {
      store,
      timeline: timeline as never,
      media: media as never,
      mobile: overrides.mobile ?? false,
      focused: overrides.focused ?? document.body,
    },
  };
}

function stubHistory(store: Store) {
  const undo = vi.fn(async () => {});
  const redo = vi.fn(async () => {});
  store.setState({ undo, redo });
  return { undo, redo };
}

describe("editorNativeMenuState", () => {
  beforeEach(() => window.localStorage.clear());

  it("enables timeline items from the planners' non-blocked results", () => {
    const { store } = createStore();
    const idle = editorNativeMenuState(store.getState(), false);
    expect(idle).toEqual({
      view: "editor",
      canImport: true,
      canExport: true,
      canUndo: false,
      canRedo: false,
      canSplit: false,
      canTrimStart: false,
      canTrimEnd: false,
      canDelete: false,
      canRippleDelete: false,
      canSelectForward: false,
    });

    selectClipUnderPlayhead(store);
    expect(editorNativeMenuState(store.getState(), false)).toMatchObject({
      canSplit: true,
      canTrimStart: true,
      canTrimEnd: true,
      canDelete: true,
      canRippleDelete: true,
      canSelectForward: true,
    });

    // On the clip's start edge, the playhead-based planners block while the selection ones still run.
    store.getState().seek(fixtureItem(store.getState().project, "video").startSeconds);
    expect(editorNativeMenuState(store.getState(), false)).toMatchObject({ canSplit: false, canTrimStart: false, canDelete: true });
  });

  it("enables Delete for a selected transition", () => {
    const project = fixtureProject();
    const video = project.timeline.tracks.find((track) => track.kind === "video");
    const [left, right] = video?.items ?? [];
    if (!video || !left || !right) throw new Error("sample video clips");
    video.transitions = [{ id: "fade", leftItemId: left.id, rightItemId: right.id, kind: "crossfade", durationSeconds: 0.5 }];
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().selectTransition("fade");
    expect(editorNativeMenuState(store.getState(), false)).toMatchObject({ canDelete: true, canRippleDelete: true, canSplit: false });
  });

  it("offers Undo for an agent batch and for a focused text field", async () => {
    const { store, project } = createStore();
    await act(() => store.getState().commitAgentApply({ ...project, contentRevision: 2 }, "message-1"));
    expect(store.getState().history.past).toHaveLength(0);
    expect(editorNativeMenuState(store.getState(), false)).toMatchObject({ canUndo: true, canRedo: false });

    const fresh = createStore().store;
    expect(editorNativeMenuState(fresh.getState(), true)).toMatchObject({ canUndo: true, canRedo: true });
  });

  it("needs a project folder to import or export", () => {
    expect(editorNativeMenuState(createStore("").store.getState(), false)).toMatchObject({ canImport: false, canExport: false });
  });
});

describe("runNativeEditorCommand", () => {
  beforeEach(() => {
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    window.localStorage.clear();
  });

  it("routes Undo and Redo to the editor's global undo when no text field has focus", () => {
    const { store } = createStore();
    const history = stubHistory(store);
    const { context } = commandContext(store);

    runNativeEditorCommand("undo", context);
    runNativeEditorCommand("redo", context);

    expect(history.undo).toHaveBeenCalledTimes(1);
    expect(history.redo).toHaveBeenCalledTimes(1);
  });

  it("gives a focused text field its own undo instead of the project's", () => {
    const { store } = createStore();
    const history = stubHistory(store);
    const execCommand = vi.fn(() => true);
    Object.defineProperty(document, "execCommand", { configurable: true, value: execCommand });
    const input = document.createElement("textarea");
    const { context } = commandContext(store, { focused: input });

    runNativeEditorCommand("undo", context);
    runNativeEditorCommand("redo", context);

    expect(execCommand.mock.calls).toEqual([["undo"], ["redo"]]);
    expect(history.undo).not.toHaveBeenCalled();
    expect(history.redo).not.toHaveBeenCalled();
    Reflect.deleteProperty(document, "execCommand");
  });

  it("opens tabs, sheets on phones, overlays and the Export popover", () => {
    const { store } = createStore();
    runNativeEditorCommand("showTab:captions", commandContext(store).context);
    expect(store.getState()).toMatchObject({ activeTab: "captions", openSheetId: null });

    runNativeEditorCommand("showTab:effects", commandContext(store, { mobile: true }).context);
    expect(store.getState()).toMatchObject({ activeTab: "effects", openSheetId: "effects" });

    const overlays = [
      ["openShortcuts", "shortcuts"],
      ["openConnectAgents", "connectAgents"],
      ["openProjectGuidance", "projectSkills"],
    ] as const;
    for (const [command, overlay] of overlays) {
      runNativeEditorCommand(command, commandContext(store).context);
      expect(store.getState().overlay).toBe(overlay);
    }

    runNativeEditorCommand("exportProject", commandContext(store).context);
    expect(store.getState().exportPopover).toEqual({ preset: null });
  });

  it("imports through the media service from the Media tab", () => {
    const { store } = createStore();
    const { context, media } = commandContext(store);
    runNativeEditorCommand("importMedia", context);
    expect(store.getState().activeTab).toBe("media");
    expect(media.importMediaFiles).toHaveBeenCalledWith();
  });

  it("runs timeline edits through the same commands as their shortcuts", () => {
    const { store } = createStore();
    const item = selectClipUnderPlayhead(store);
    const { context, timeline } = commandContext(store);

    runNativeEditorCommand("split", context);
    runNativeEditorCommand("trimStart", context);
    runNativeEditorCommand("trimEnd", context);
    runNativeEditorCommand("delete", context);
    runNativeEditorCommand("rippleDelete", context);
    expect(timeline.splitAtPlayhead).toHaveBeenCalledTimes(1);
    expect(timeline.trimToPlayhead.mock.calls).toEqual([["start"], ["end"]]);
    expect(timeline.deleteSelection).toHaveBeenCalledTimes(1);
    expect(timeline.rippleDeleteSelection).toHaveBeenCalledTimes(1);

    runNativeEditorCommand("selectForwardAll", context);
    expect(store.getState().selectedItemIds).toContain(item.id);
    // App-level commands are App's to handle.
    runNativeEditorCommand("openSettings", context);
    expect(store.getState().overlay).toBeNull();
  });
});

describe("useNativeMenuBridge", () => {
  let reported: NativeMenuState[];

  beforeEach(() => {
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    window.localStorage.clear();
    reported = [];
  });

  afterEach(() => {
    document.body.replaceChildren();
    Reflect.deleteProperty(document, "execCommand");
  });

  function renderBridge(store: Store, initial: { isActive: boolean; request: NativeMenuRequest | null }, withShortcuts = false) {
    const onStateChange = (state: NativeMenuState) => reported.push(state);
    const wrapper = ({ children }: { children: ReactNode }) => <EditorStoreProvider store={store}>{children}</EditorStoreProvider>;
    return renderHook(
      (props: { isActive: boolean; request: NativeMenuRequest | null }) => {
        if (withShortcuts) useEditorShortcuts("macos");
        useNativeMenuBridge({ ...props, mobile: false, onStateChange });
      },
      { wrapper, initialProps: initial },
    );
  }

  it("reports only while active and only when a capability changes", () => {
    const { store } = createStore();
    const view = renderBridge(store, { isActive: false, request: null });
    expect(reported).toEqual([]);

    view.rerender({ isActive: true, request: null });
    expect(reported).toHaveLength(1);
    expect(reported[0]).toMatchObject({ view: "editor", canSplit: false });

    act(() => store.getState().setPlaying(true));
    expect(reported).toHaveLength(1);

    act(() => void selectClipUnderPlayhead(store));
    expect(reported[reported.length - 1]).toMatchObject({ canSplit: true, canDelete: true });
    const count = reported.length;

    view.rerender({ isActive: false, request: null });
    act(() => store.getState().clearSelection());
    expect(reported).toHaveLength(count);
  });

  it("keeps Undo enabled while a text field has focus", async () => {
    const { store } = createStore();
    renderBridge(store, { isActive: true, request: null });
    const input = document.createElement("input");
    document.body.append(input);

    act(() => input.focus());
    await vi.waitFor(() => expect(reported[reported.length - 1]).toMatchObject({ canUndo: true, canRedo: true }));
    act(() => input.blur());
    await vi.waitFor(() => expect(reported[reported.length - 1]).toMatchObject({ canUndo: false, canRedo: false }));
  });

  it("runs each forwarded request once and ignores one left from an earlier session", () => {
    const { store } = createStore();
    const history = stubHistory(store);
    const stale = { sequence: 4, command: "undo" } as const;
    const view = renderBridge(store, { isActive: true, request: stale });
    expect(history.undo).not.toHaveBeenCalled();

    const next = { sequence: 5, command: "undo" } as const;
    view.rerender({ isActive: true, request: next });
    // Returning from Settings re-renders with the same request; it must not undo again.
    view.rerender({ isActive: false, request: null });
    view.rerender({ isActive: true, request: next });
    expect(history.undo).toHaveBeenCalledTimes(1);
  });

  it("runs one undo per ⌘Z: the web view handles it, or the menu does when a field skipped it", async () => {
    const { store } = createStore();
    const history = stubHistory(store);
    const execCommand = vi.fn(() => true);
    Object.defineProperty(document, "execCommand", { configurable: true, value: execCommand });
    const view = renderBridge(store, { isActive: true, request: null }, true);

    // Nothing focused: the keydown handler undoes and claims the key, so macOS never sends it to the menu.
    const claimed = new KeyboardEvent("keydown", { key: "z", metaKey: true, cancelable: true });
    act(() => void window.dispatchEvent(claimed));
    expect(claimed.defaultPrevented).toBe(true);
    expect(history.undo).toHaveBeenCalledTimes(1);

    // In a text field the handler leaves the key alone; the menu item's command then undoes the field.
    const input = document.createElement("input");
    document.body.append(input);
    act(() => input.focus());
    const skipped = new KeyboardEvent("keydown", { key: "z", metaKey: true, cancelable: true, bubbles: true });
    act(() => void input.dispatchEvent(skipped));
    expect(skipped.defaultPrevented).toBe(false);
    view.rerender({ isActive: true, request: { sequence: 1, command: "undo" } });

    expect(history.undo).toHaveBeenCalledTimes(1);
    expect(execCommand).toHaveBeenCalledTimes(1);
    expect(execCommand).toHaveBeenCalledWith("undo");
  });
});
