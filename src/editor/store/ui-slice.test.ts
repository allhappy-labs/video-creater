import { beforeEach, describe, expect, it, vi } from "vitest";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: vi.fn(),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => path,
}));

const { createEditorStore } = await import("./editor-store");

function store() {
  const project = fixtureProject();
  return { project, store: createEditorStore({ projectDir: "/p", project }) };
}

describe("ui slice properties state", () => {
  beforeEach(() => window.localStorage.clear());

  it("sets and clears the live property preview without touching the project", () => {
    const { project, store: editor } = store();
    const itemId = fixtureItem(project, "video").id;
    const before = editor.getState().project;
    expect(editor.getState().propertyPreview).toBeNull();

    editor.getState().setPropertyPreview({ itemId, patch: { opacity: 0.4 } });
    expect(editor.getState().propertyPreview).toEqual({ itemId, patch: { opacity: 0.4 } });
    expect(editor.getState().project).toBe(before);
    expect(editor.getState().history.past).toHaveLength(0);

    editor.getState().clearPropertyPreview();
    expect(editor.getState().propertyPreview).toBeNull();
  });

  it("remembers the active properties tab per selection kind", () => {
    const { store: editor } = store();
    expect(editor.getState().propertiesTabByKind).toEqual({});

    editor.getState().setPropertiesTab("visual", "speed");
    editor.getState().setPropertiesTab("caption", "style");
    editor.getState().setPropertiesTab("visual", "animation");

    expect(editor.getState().propertiesTabByKind).toEqual({ visual: "animation", caption: "style" });
  });

  it("sets and clears the crop mode item for the canvas", () => {
    const { project, store: editor } = store();
    const itemId = fixtureItem(project, "video").id;
    expect(editor.getState().cropModeItemId).toBeNull();
    editor.getState().setCropModeItemId(itemId);
    expect(editor.getState().cropModeItemId).toBe(itemId);
    editor.getState().setCropModeItemId(null);
    expect(editor.getState().cropModeItemId).toBeNull();
  });
});

describe("ui slice toasts", () => {
  beforeEach(() => window.localStorage.clear());

  it("queues toasts with an optional action, keeps the newest three, and dismisses by id", () => {
    const { store: editor } = store();
    const onSelect = vi.fn();
    const first = editor.getState().pushToast({ title: "Exported Intro", action: { label: "Undo", onSelect } });
    expect(editor.getState().toasts).toEqual([{ id: first, title: "Exported Intro", action: { label: "Undo", onSelect } }]);

    const ids = [2, 3, 4].map((n) => editor.getState().pushToast({ title: `Exported ${n}` }));
    expect(editor.getState().toasts.map((toast) => toast.title)).toEqual(["Exported 2", "Exported 3", "Exported 4"]);

    editor.getState().dismissToast(ids[1] ?? "");
    expect(editor.getState().toasts.map((toast) => toast.title)).toEqual(["Exported 2", "Exported 4"]);
  });
});

describe("ui slice overlays", () => {
  beforeEach(() => window.localStorage.clear());

  it("opens one gear-menu overlay at a time and closes it", () => {
    const { store: editor } = store();
    expect(editor.getState().overlay).toBeNull();
    editor.getState().openOverlay("shortcuts");
    expect(editor.getState().overlay).toBe("shortcuts");
    editor.getState().openOverlay("connectAgents");
    expect(editor.getState().overlay).toBe("connectAgents");
    editor.getState().closeOverlay();
    expect(editor.getState().overlay).toBeNull();
  });
});
