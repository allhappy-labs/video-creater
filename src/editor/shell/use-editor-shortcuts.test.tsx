import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import type { ProjectAction } from "@/lib/project";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (p: string) => p,
}));

const { createEditorStore } = await import("../store/editor-store");
const { EditorStoreProvider } = await import("../store/editor-store-context");
const { useEditorShortcuts } = await import("./use-editor-shortcuts");

function setup(platform: "macos" | "linux") {
  const project = { ...fixtureProject(), schemaVersion: 2, contentRevision: 1 };
  const store = createEditorStore({ projectDir: "/p", project });
  const wrapper = ({ children }: { children: ReactNode }) => <EditorStoreProvider store={store}>{children}</EditorStoreProvider>;
  renderHook(() => useEditorShortcuts(platform), { wrapper });
  return { store, project };
}

describe("useEditorShortcuts", () => {
  beforeEach(() => {
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    window.localStorage.clear();
  });

  it("opens the Export popover with Mod+E", () => {
    const { store } = setup("macos");
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "e", metaKey: true }));
    });
    expect(store.getState().exportPopover).toEqual({ preset: null });
  });

  it("opens the Keyboard shortcuts sheet with Mod+/", () => {
    const { store } = setup("linux");
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "/", ctrlKey: true }));
    });
    expect(store.getState().overlay).toBe("shortcuts");
  });

  it("switches tabs with Mod+number", () => {
    const { store } = setup("linux");
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "5", ctrlKey: true }));
    });
    expect(store.getState().activeTab).toBe("captions");
  });

  it("undoes with Mod+Z and ignores editable targets", async () => {
    const { store, project } = setup("macos");
    await act(async () => {
      await store.getState().applyActions([
        { type: "updateVisualClipOpacity", itemId: fixtureItem(project, "video").id, opacity: 0.25 } as ProjectAction,
      ]);
    });
    const input = document.createElement("input");
    document.body.append(input);
    await act(async () => {
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true, bubbles: true }));
    });
    expect(store.getState().history.past).toHaveLength(1);

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(store.getState().history.past).toHaveLength(0));
    input.remove();
  });

  it("clears selection with Escape", () => {
    const { store, project } = setup("linux");
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(store.getState().selectedItemIds).toEqual([]);
  });

  it("backs out of the window-filling preview, then asset preview, before clearing the selection", () => {
    const { store, project } = setup("linux");
    const itemId = fixtureItem(project, "video").id;
    act(() => {
      store.getState().selectItems([itemId]);
      store.getState().previewAsset("media-1");
      store.getState().setFullscreen(true);
    });
    const escape = () => act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    escape();
    expect(store.getState()).toMatchObject({ fullscreen: false, previewSource: { kind: "asset", mediaId: "media-1" } });
    escape();
    expect(store.getState()).toMatchObject({ previewSource: { kind: "timeline" }, selectedItemIds: [itemId] });
    escape();
    expect(store.getState().selectedItemIds).toEqual([]);
  });

  it("toggles preview playback with Space only when nothing is focused", () => {
    const { store } = setup("linux");
    act(() => void document.body.dispatchEvent(new KeyboardEvent("keydown", { key: " ", bubbles: true })));
    expect(store.getState().playing).toBe(true);

    const button = document.createElement("button");
    document.body.append(button);
    act(() => void button.dispatchEvent(new KeyboardEvent("keydown", { key: " ", bubbles: true })));
    expect(store.getState().playing).toBe(true);
    button.remove();
  });
});
