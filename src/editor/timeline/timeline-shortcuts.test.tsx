import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { editorShortcuts } from "@/lib/keymap";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { installPointerEventPolyfill, renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { TimelinePanel } from "./timeline-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items };
}

/** 24 fps, 80 px/s after the 118 px header: Video 1 (0–58 px) "a" 0–2 s and "b" 4–6 s; Video 2 (58–92 px) "c" 0–1 s; Audio 1 "m". */
function projectWith(): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: [
      track("v1", "video", [clip("a", 0, 2, { properties: { sourceIn: 0, sourceOut: 2 } }), clip("b", 4, 2)]),
      track("v2", "video", [clip("c", 0, 1)]),
      track("a1", "audio", [clip("m", 0, 3, { kind: "audio_clip" })]),
    ],
  };
  return project;
}

function renderPanel() {
  return renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
}

function canvas(): HTMLElement {
  return screen.getByRole("region", { name: "Timeline canvas" });
}

function option(itemId: string): HTMLElement {
  const element = screen.getAllByRole("option").find((candidate) => candidate.dataset.itemId === itemId);
  if (!element) throw new Error(`no option for ${itemId}`);
  return element;
}

/** Presses a key on the focused canvas (the macOS default in jsdom: Mod is ⌘). Returns false when handled. */
async function press(key: string, init: { shiftKey?: boolean; metaKey?: boolean } = {}, target: Element = canvas()) {
  let notPrevented = true;
  await act(async () => {
    notPrevented = fireEvent.keyDown(target, { key, ...init });
    await Promise.resolve();
  });
  return notPrevented;
}

function item(project: VideoProject, itemId: string) {
  return project.timeline.tracks.flatMap((entry) => entry.items).find((entry) => entry.id === itemId);
}

function eventInit(binding: string) {
  const parts = binding.split("+");
  const key = parts.at(-1) ?? "";
  return { key: key === "Space" ? " " : key.length === 1 ? key.toLowerCase() : key, init: { shiftKey: parts.includes("Shift"), metaKey: parts.includes("Mod") } };
}

describe("timeline shortcuts", () => {
  beforeAll(() => installPointerEventPolyfill());
  beforeEach(() => window.localStorage.clear());

  it("handles every timeline-scope shortcut in the keymap", async () => {
    renderPanel();
    for (const shortcut of editorShortcuts.filter((entry) => entry.scope === "timeline")) {
      for (const binding of shortcut.bindings) {
        const { key, init } = eventInit(binding);
        expect({ id: shortcut.id, binding, handled: !(await press(key, init)) }).toEqual({ id: shortcut.id, binding, handled: true });
      }
    }
  });

  it("switches tools, splits at the playhead and toggles playback", async () => {
    const { store } = renderPanel();
    await press("c");
    expect(store.getState().tool).toBe("blade");
    await press("v");
    expect(store.getState().tool).toBe("select");

    act(() => {
      store.getState().selectItems(["a"]);
      store.getState().seek(1);
    });
    await press("s");
    expect(item(store.getState().project, "a-split-1000")).toMatchObject({ startSeconds: 1 });
    act(() => store.getState().selectItems(["b"]));
    act(() => store.getState().seek(5));
    await press("k", { metaKey: true });
    expect(item(store.getState().project, "b-split-5000")).toBeDefined();

    await press(" ");
    expect(store.getState().playing).toBe(true);
  });

  it("sets I/O range marks and drops a mark that would leave a range under 0.25 s", async () => {
    const { store } = renderPanel();
    act(() => store.getState().seek(2));
    await press("i");
    act(() => store.getState().seek(5));
    await press("o");
    expect(store.getState()).toMatchObject({ rangeIn: 2, rangeOut: 5 });
    act(() => store.getState().seek(4.9));
    await press("i");
    expect(store.getState()).toMatchObject({ rangeIn: 4.9, rangeOut: null });
  });

  it("deletes a selected gap before clips, and ripple deletes with Shift", async () => {
    const { store } = renderPanel();
    // Click empty Video 1 space at 3 s, between "a" and "b".
    fireEvent.pointerDown(screen.getByRole("listbox", { name: "Video 1" }), { button: 0, clientX: 118 + 240, clientY: 20 });
    fireEvent.pointerUp(window, { clientX: 118 + 240, clientY: 20 });
    expect(store.getState().selectedGap).toEqual({ trackId: "v1", startSeconds: 2, endSeconds: 4 });
    expect(screen.getByTestId("timeline-selected-gap")).toHaveStyle({ left: "160px", width: "160px" });

    await press("Delete");
    expect(item(store.getState().project, "b")?.startSeconds).toBe(2);
    expect(store.getState().selectedGap).toBeNull();

    act(() => store.getState().selectItems(["a"]));
    await press("Backspace", { shiftKey: true });
    expect(item(store.getState().project, "a")).toBeUndefined();
    expect(item(store.getState().project, "b")?.startSeconds).toBe(0);

    act(() => store.getState().selectItems(["c"]));
    await press("Delete");
    expect(store.getState().project.timeline.tracks.map((entry) => entry.id)).toEqual(["v1", "a1"]);
  });

  it("copies, cuts, duplicates and pastes at the playhead", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["b"]));
    await press("c", { metaKey: true });
    act(() => store.getState().seek(10));
    await press("v", { metaKey: true });
    expect(item(store.getState().project, "b-copy")).toMatchObject({ startSeconds: 10 });

    act(() => store.getState().selectItems(["a"]));
    await press("d", { metaKey: true });
    expect(item(store.getState().project, "a-copy")).toMatchObject({ startSeconds: 2 });

    act(() => store.getState().selectItems(["c"]));
    await press("x", { metaKey: true });
    expect(item(store.getState().project, "c")).toBeUndefined();
    act(() => store.getState().seek(1));
    await press("v", { metaKey: true, shiftKey: true });
    const inserted = store.getState().project.timeline.tracks.flatMap((entry) => entry.items).find((entry) => entry.label === "c insert");
    expect(inserted).toMatchObject({ startSeconds: 1 });
  });

  it("trims to the playhead, nudges, and moves clips between tracks", async () => {
    const { store } = renderPanel();
    act(() => {
      store.getState().selectItems(["a"]);
      store.getState().seek(0.5);
    });
    await press("[");
    expect(item(store.getState().project, "a")).toMatchObject({ startSeconds: 0.5, durationSeconds: 1.5, properties: { sourceIn: 0.5 } });
    act(() => store.getState().seek(1.5));
    await press("]");
    expect(item(store.getState().project, "a")).toMatchObject({ durationSeconds: 1 });

    act(() => store.getState().selectItems(["b"]));
    await press("ArrowRight", { shiftKey: true });
    expect(item(store.getState().project, "b")?.startSeconds).toBeCloseTo(4 + 1 / 24, 3);
    await press("ArrowDown", { shiftKey: true });
    expect(store.getState().project.timeline.tracks.find((entry) => entry.id === "v2")?.items.map((entry) => entry.id)).toEqual(["c", "b"]);
  });

  it("navigates the playhead and selects forward", async () => {
    const { store } = renderPanel();
    await press("End");
    expect(store.getState().playheadSeconds).toBe(30);
    await press("Home");
    expect(store.getState().playheadSeconds).toBe(0);
    await press("ArrowRight");
    expect(store.getState().playheadSeconds).toBe(0.25);
    await press("PageDown");
    expect(store.getState().playheadSeconds).toBe(1);
    await press("PageUp");
    expect(store.getState().playheadSeconds).toBe(0);

    act(() => store.getState().selectItems(["a"]));
    await press("a");
    expect(store.getState().selectedItemIds).toEqual(["a", "b"]);
    await press("a", { shiftKey: true });
    expect(store.getState().selectedItemIds).toEqual(["a", "b", "c", "m"]);
  });

  it("resets the tool, range and gap on Escape and leaves the selection to the global handler", async () => {
    const { store } = renderPanel();
    act(() => {
      store.getState().setTool("blade");
      store.getState().setRangeIn(1);
      store.getState().selectItems(["a"]);
      store.getState().selectGap({ trackId: "v1", startSeconds: 2, endSeconds: 4 });
    });
    expect(await press("Escape")).toBe(true);
    expect(store.getState()).toMatchObject({ tool: "select", rangeIn: null, selectedGap: null, marquee: null });
  });

  it("ignores keys in editable fields, portalled menus and keys a focused clip handled", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["b"]));
    fireEvent.contextMenu(option("b"), { clientX: 400, clientY: 20 });
    const menu = await screen.findByRole("menu", { name: "Clip actions" });
    expect(await press("s", {}, menu)).toBe(true);

    fireEvent.click(within(menu).getByRole("menuitem", { name: "Set duration…" }));
    const input = await screen.findByRole("textbox", { name: "Duration" });
    expect(await press("s", {}, input)).toBe(true);
    expect(await press("Delete", {}, input)).toBe(true);
    expect(item(store.getState().project, "b")).toBeDefined();
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    act(() => store.getState().seek(0));
    await press("ArrowRight", {}, option("a"));
    expect(store.getState().playheadSeconds).toBe(0);
    expect(document.activeElement).toBe(option("b"));
  });
});
