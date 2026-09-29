import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { installPointerEventPolyfill, renderWithEditorStore, stubRect } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { TimelinePanel } from "./timeline-panel";

const { appliedBatches } = vi.hoisted(() => ({ appliedBatches: [] as ProjectAction[][] }));

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("@/lib/agent/project-merge", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/agent/project-merge")>();
  return {
    ...actual,
    applyProjectActionsLocally: (project: VideoProject, actions: ProjectAction[]) => {
      appliedBatches.push(actions);
      return actual.applyProjectActionsLocally(project, actions);
    },
  };
});

/** Queued animation frames, flushed explicitly so coalesced pointer moves run synchronously. */
let frames: FrameRequestCallback[] = [];

function flushFrames() {
  act(() => {
    const pending = frames;
    frames = [];
    for (const callback of pending) callback(0);
  });
}

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[], patch: Partial<TimelineTrack> = {}): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items, ...patch };
}

/**
 * Rows at 80 px/s after the 118 px header: v1 (main) 0–58 px, a1 58–92 px.
 * "a" 0–2 s (x 118–278), "b" 4–6 s (x 438–598), "m" 0–3 s on audio.
 */
function projectWith(tracks?: TimelineTrack[]): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: tracks ?? [
      track("v1", "video", [clip("a", 0, 2, { properties: { sourceIn: 0, sourceOut: 2 } }), clip("b", 4, 2)]),
      track("a1", "audio", [clip("m", 0, 3, { kind: "audio_clip", source: { type: "media", mediaId: "media-voiceover" } })]),
    ],
  };
  return project;
}

function renderPanel(project = projectWith()) {
  const rendered = renderWithEditorStore(<TimelinePanel />, { project });
  return rendered;
}

function option(itemId: string): HTMLElement {
  const element = screen.getAllByRole("option").find((candidate) => candidate.dataset.itemId === itemId);
  if (!element) throw new Error(`no option for ${itemId}`);
  return element;
}

function drag(element: Element, from: { x: number; y: number }, to: { x: number; y: number }, init: PointerEventInit = {}) {
  fireEvent.pointerDown(element, { button: 0, clientX: from.x, clientY: from.y, ...init });
  fireEvent.pointerMove(window, { clientX: to.x, clientY: to.y });
  flushFrames();
}

function release(at: { x: number; y: number }) {
  fireEvent.pointerUp(window, { clientX: at.x, clientY: at.y });
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

function startOf(project: VideoProject, itemId: string): number | undefined {
  return project.timeline.tracks.flatMap((entry) => entry.items).find((item) => item.id === itemId)?.startSeconds;
}

describe("timeline clip interactions", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    appliedBatches.length = 0;
    frames = [];
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => frames.push(callback));
    vi.stubGlobal("cancelAnimationFrame", () => undefined);
  });

  afterEach(() => vi.unstubAllGlobals());

  it("selects on press and commits a drag as one moveItems batch", async () => {
    const { store } = renderPanel();
    store.getState().setSnapEnabled(false);
    drag(option("a"), { x: 150, y: 29 }, { x: 230, y: 29 });
    expect(store.getState().selectedItemIds).toEqual(["a"]);
    expect(screen.getByTestId("timeline-interaction-tooltip")).toHaveTextContent("00:00:01 · 00:00:02");
    expect(option("a")).toHaveAttribute("data-dragging", "true");

    release({ x: 230, y: 29 });
    await settle();
    expect(appliedBatches).toEqual([[{ type: "moveItems", moves: [{ itemId: "a", targetTrackId: "v1", startSeconds: 1 }] }]]);
    expect(startOf(store.getState().project, "a")).toBe(1);
    expect(store.getState().history.past).toHaveLength(1);
    expect(screen.queryByTestId("clip-drag-ghost")).not.toBeInTheDocument();
  });

  it("does not move under the 3 px threshold and collapses a multi-selection on click", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a", "b"]));
    drag(option("a"), { x: 150, y: 29 }, { x: 152, y: 30 });
    expect(screen.queryByTestId("clip-drag-ghost")).not.toBeInTheDocument();
    release({ x: 152, y: 30 });
    await settle();
    expect(appliedBatches).toEqual([]);
    expect(store.getState().selectedItemIds).toEqual(["a"]);
  });

  it("moves the playhead to the clicked point when a click selects a clip", () => {
    const { store } = renderPanel();
    const target = option("b");
    stubRect(target, { left: 438, width: 160, height: 52 });
    fireEvent.pointerDown(target, { button: 0, clientX: 438 + 60, clientY: 29 });
    release({ x: 438 + 61, y: 29 });
    expect(store.getState().selectedItemIds).toEqual(["b"]);
    expect(store.getState().playheadSeconds).toBe(4.75);
  });

  it("does not seek on a click while playback runs", () => {
    const { store } = renderPanel();
    act(() => store.getState().setPlaying(true));
    fireEvent.pointerDown(option("b"), { button: 0, clientX: 60, clientY: 29 });
    release({ x: 60, y: 29 });
    expect(store.getState().selectedItemIds).toEqual(["b"]);
    expect(store.getState().playheadSeconds).toBe(0);
  });

  it("does not seek when a press turns into a drag", async () => {
    const { store } = renderPanel();
    store.getState().setSnapEnabled(false);
    drag(option("a"), { x: 150, y: 29 }, { x: 230, y: 29 });
    release({ x: 230, y: 29 });
    await settle();
    expect(startOf(store.getState().project, "a")).toBe(1);
    expect(store.getState().playheadSeconds).toBe(0);
  });

  it("does not seek when a clip is selected from the keyboard", () => {
    const { store } = renderPanel();
    fireEvent.keyDown(option("b"), { key: "Enter" });
    fireEvent.click(option("a"));
    expect(store.getState().selectedItemIds).toEqual(["a"]);
    expect(store.getState().playheadSeconds).toBe(0);
  });

  it("toggles the selection with a modifier", () => {
    const { store } = renderPanel();
    fireEvent.pointerDown(option("a"), { button: 0, clientX: 150, clientY: 29 });
    release({ x: 150, y: 29 });
    fireEvent.pointerDown(option("b"), { button: 0, clientX: 450, clientY: 29, metaKey: true });
    release({ x: 450, y: 29 });
    expect(store.getState().selectedItemIds).toEqual(["a", "b"]);
  });

  it("snaps to the playhead within 8 px and shows the guide", async () => {
    const { store } = renderPanel();
    act(() => store.getState().seek(3));
    // 84 px (1.05 s) of travel puts the end of "a" at 3.05 s, 4 px from the playhead at 3 s.
    drag(option("a"), { x: 150, y: 29 }, { x: 234, y: 29 });
    expect(screen.getByTestId("timeline-snap-guide")).toHaveStyle({ left: "240px" });
    release({ x: 234, y: 29 });
    await settle();
    expect(startOf(store.getState().project, "a")).toBe(1);
  });

  it("shows a rejected collision at the target with its reason and commits nothing", async () => {
    const { store } = renderPanel();
    store.getState().setSnapEnabled(false);
    drag(option("a"), { x: 150, y: 29 }, { x: 430, y: 29 });
    expect(screen.getByTestId("timeline-interaction-tooltip")).toHaveTextContent("Overlaps b");
    expect(screen.getByTestId("clip-drag-ghost")).toHaveAttribute("data-rejected", "true");
    expect(screen.getByRole("listbox", { name: "Video 1" }).parentElement).toHaveAttribute("data-drop-target", "rejected");

    release({ x: 430, y: 29 });
    await settle();
    expect(appliedBatches).toEqual([]);
    expect(store.getState().history.past).toHaveLength(0);
    expect(store.getState().lastError).toBe("Overlaps b");
  });

  it("drags into a new track between rows as one batch with createTrack", async () => {
    const project = projectWith([track("v1", "video", [clip("a", 0, 2)]), track("v2", "video", [clip("b", 0, 2)])]);
    const { store } = renderPanel(project);
    store.getState().setSnapEnabled(false);
    // v1 0–58, v2 58–92: dropping "b" with its centre 2 px below the top edge of v1 inserts above v1.
    drag(option("b"), { x: 150, y: 75 }, { x: 150, y: 2 });
    expect(screen.getByTestId("timeline-insert-line")).toHaveStyle({ top: "0px" });
    release({ x: 150, y: 2 });
    await settle();
    expect(appliedBatches).toHaveLength(1);
    expect(appliedBatches[0]?.map((action) => action.type)).toEqual(["createTrack", "reorderTrack", "moveItems", "removeTracks"]);
    expect(store.getState().project.timeline.tracks.map((entry) => entry.items.map((item) => item.id))).toEqual([["b"], ["a"]]);
  });

  it("keeps the dragged clip on its row when selecting it moves the keyframe lane above it", async () => {
    const project = projectWith();
    project.timeline.tracks[1]?.items.push(clip("n", 10, 2, { kind: "audio_clip" }));
    const { store } = renderPanel(project);
    store.getState().setSnapEnabled(false);
    act(() => {
      store.getState().setKeyframesVisible(true);
      store.getState().selectItems(["a"]);
    });
    // With the lane under Video 1 the audio row sits at 98–132 px; pressing "m" moves the lane under it.
    drag(option("m"), { x: 150, y: 115 }, { x: 390, y: 115 });
    release({ x: 390, y: 115 });
    await settle();
    expect(appliedBatches).toEqual([[{ type: "moveItems", moves: [{ itemId: "m", targetTrackId: "a1", startSeconds: 3 }] }]]);
  });

  it("duplicates with Alt and selects the copy", async () => {
    const { store } = renderPanel();
    store.getState().setSnapEnabled(false);
    drag(option("a"), { x: 150, y: 29 }, { x: 310, y: 29 }, { altKey: true });
    expect(option("a")).not.toHaveAttribute("data-dragging");
    expect(screen.getByTestId("clip-drag-ghost")).toHaveTextContent("a copy");
    release({ x: 310, y: 29 });
    await settle();
    expect(appliedBatches[0]?.map((action) => action.type)).toEqual(["addItems"]);
    expect(startOf(store.getState().project, "a")).toBe(0);
    expect(startOf(store.getState().project, "a-copy")).toBe(2);
    expect(store.getState().selectedItemIds).toEqual(["a-copy"]);
  });

  it("cancels a drag with Escape", async () => {
    renderPanel();
    drag(option("a"), { x: 150, y: 29 }, { x: 230, y: 29 });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByTestId("clip-drag-ghost")).not.toBeInTheDocument();
    release({ x: 230, y: 29 });
    await settle();
    expect(appliedBatches).toEqual([]);
  });

  it("renders labelled trim handles only for the selected clip", () => {
    const { store } = renderPanel();
    const canvas = screen.getByRole("region", { name: "Timeline canvas" });
    expect(within(canvas).queryByRole("slider")).not.toBeInTheDocument();
    act(() => store.getState().selectItems(["a"]));
    const left = screen.getByRole("slider", { name: "Resize a left edge" });
    expect(left).toHaveAttribute("aria-valuenow", "0");
    expect(left).toHaveAttribute("aria-valuetext", "Left edge at 0.00 seconds; duration 2.00 seconds");
    expect(screen.getByRole("slider", { name: "Resize a right edge" })).toHaveAttribute("aria-valuenow", "2");
    expect(within(screen.getByRole("listbox", { name: "Video 1" })).queryByRole("slider")).not.toBeInTheDocument();
  });

  it("trims the left edge into trimItems with an updated sourceIn", async () => {
    const { store } = renderPanel();
    store.getState().setSnapEnabled(false);
    act(() => store.getState().selectItems(["a"]));
    drag(screen.getByRole("slider", { name: "Resize a left edge" }), { x: 120, y: 29 }, { x: 160, y: 29 });
    expect(option("a")).toHaveStyle({ left: "40px", width: "120px" });
    expect(screen.getByTestId("timeline-interaction-tooltip")).toHaveTextContent("00:00:00.500 · 00:00:01.500");
    release({ x: 160, y: 29 });
    await settle();
    expect(appliedBatches).toEqual([
      [{ type: "trimItems", trims: [{ itemId: "a", startSeconds: 0.5, durationSeconds: 1.5, sourceIn: 0.5, sourceOut: 2 }] }],
    ]);
    expect(store.getState().selectedItemIds).toEqual(["a"]);
  });

  it("ripple trims with Shift on a handle and shifts later clips", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a"]));
    drag(screen.getByRole("slider", { name: "Resize a right edge" }), { x: 275, y: 29 }, { x: 235, y: 29 }, { shiftKey: true });
    expect(option("b")).toHaveStyle({ left: "280px" });
    release({ x: 235, y: 29 });
    await settle();
    expect(appliedBatches[0]).toEqual([
      { type: "rippleTrimItem", itemId: "a", edge: "right", deltaSeconds: -0.5, propagateLinked: true, syncLockedTrackIds: [] },
    ]);
    expect(startOf(store.getState().project, "b")).toBe(3.5);
  });

  it("trims from the keyboard by 0.25 s", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["b"]));
    fireEvent.keyDown(screen.getByRole("slider", { name: "Resize b right edge" }), { key: "ArrowRight" });
    await settle();
    expect(appliedBatches).toEqual([[{ type: "resizeItems", resizes: [{ itemId: "b", durationSeconds: 2.25 }] }]]);
  });

  it("selects the clips inside a marquee drawn from empty lane space", async () => {
    const { store } = renderPanel();
    // From 2.875 s on the audio row up to 5.25 s on the video row: touches "m" (0–3 s) and "b" (4–6 s), not "a".
    const audioLane = screen.getByRole("listbox", { name: "Audio 1" });
    drag(audioLane, { x: 118 + 230, y: 80 }, { x: 118 + 420, y: 20 });
    expect(store.getState().marquee).toEqual({ startX: 230, startY: 80, endX: 420, endY: 20 });
    expect(screen.getByTestId("timeline-marquee")).toHaveStyle({ left: "230px", top: "20px", width: "190px", height: "60px" });
    release({ x: 118 + 420, y: 20 });
    expect(store.getState().selectedItemIds).toEqual(["b", "m"]);
    expect(store.getState().marquee).toBeNull();
  });

  it("clears the selection when empty lane space is clicked", () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a"]));
    fireEvent.pointerDown(screen.getByRole("listbox", { name: "Video 1" }), { button: 0, clientX: 118 + 700, clientY: 20 });
    release({ x: 118 + 700, y: 20 });
    expect(store.getState().selectedItemIds).toEqual([]);
  });

  it("splits a clip at the pointer with the blade tool", async () => {
    const { store } = renderPanel();
    act(() => store.getState().setTool("blade"));
    store.getState().setSnapEnabled(false);
    const target = option("b");
    expect(target).toHaveClass("cursor-crosshair");
    stubRect(target, { left: 438, width: 160, height: 52 });
    fireEvent.pointerDown(target, { button: 0, clientX: 438 + 60, clientY: 29 });
    await settle();
    expect(appliedBatches).toEqual([[{ type: "splitItems", splits: [{ itemId: "b", newItemId: "b-split-4750", splitSeconds: 4.75 }] }]]);
    expect(store.getState().selectedItemIds).toEqual(["b-split-4750"]);
  });
});
