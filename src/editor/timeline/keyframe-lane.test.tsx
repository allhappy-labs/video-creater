import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
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

let frames: FrameRequestCallback[] = [];

function flushFrames() {
  act(() => {
    const pending = frames;
    frames = [];
    for (const callback of pending) callback(0);
  });
}

function clip(id: string, kind: TimelineItem["kind"], startSeconds: number, properties: Record<string, unknown> = {}): TimelineItem {
  return { id, kind, startSeconds, durationSeconds: 4, source: { type: "media", mediaId: "media-1" }, label: id, properties };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items };
}

/** "a" (video, 1–5 s) has opacity keyframes at 1 s (1.0) and 3 s (0.2); "m" is audio. */
function projectWithKeyframes(): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: [
      track("v1", "video", [clip("a", "video_clip", 1, { keyframes: { opacity: [{ atSeconds: 1, value: 1 }, { atSeconds: 3, value: 0.2, easing: "easeIn" }] } }), clip("b", "video_clip", 6)]),
      track("a1", "audio", [clip("m", "audio_clip", 0)]),
    ],
  };
  return project;
}

function renderPanel() {
  const rendered = renderWithEditorStore(<TimelinePanel />, { project: projectWithKeyframes() });
  act(() => rendered.store.getState().setKeyframesVisible(true));
  return rendered;
}

function lane() {
  return screen.queryByRole("group", { name: /keyframe lane for/ });
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe("KeyframeLane", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    appliedBatches.length = 0;
    frames = [];
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => frames.push(callback));
    vi.stubGlobal("cancelAnimationFrame", () => undefined);
  });

  afterEach(() => vi.unstubAllGlobals());

  it("shows only with the keyframes toggle on and exactly one clip selected, right under its track", () => {
    const { store } = renderPanel();
    expect(lane()).not.toBeInTheDocument();

    act(() => store.getState().selectItems(["a", "b"]));
    expect(lane()).not.toBeInTheDocument();

    act(() => store.getState().selectItems(["a"]));
    expect(lane()).toHaveAccessibleName("Opacity keyframe lane for a");
    const videoRow = screen.getByRole("listbox", { name: "Video 1" }).closest(".flex");
    expect(videoRow?.nextElementSibling).toBe(screen.getByTestId("keyframe-lane-row"));
    expect(screen.getByTestId("keyframe-lane-row")).toHaveStyle({ height: "40px" });

    act(() => store.getState().setKeyframesVisible(false));
    expect(lane()).not.toBeInTheDocument();
  });

  it("lists the clip's keyframe properties in the lane menu", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a"]));
    fireEvent.keyDown(screen.getByRole("button", { name: "Keyframe property: Opacity" }), { key: "Enter" });
    const items = await screen.findAllByRole("menuitemradio");
    expect(items.map((item) => item.textContent)).toEqual(expect.arrayContaining(["Opacity", "Position X", "Scale", "Rotation"]));
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Scale" }));
    expect(store.getState().laneProperty).toBe("scale");
    expect(lane()).toHaveAccessibleName("Scale keyframe lane for a");

    act(() => store.getState().selectItems(["m"]));
    expect(lane()).toHaveAccessibleName("Volume dB keyframe lane for m");
    fireEvent.keyDown(screen.getByRole("button", { name: "Keyframe property: Volume dB" }), { key: "Enter" });
    expect((await screen.findAllByRole("menuitemradio")).map((item) => item.textContent)).toEqual(["Volume dB"]);
  });

  it("renders focusable, labelled diamonds positioned by time and value", () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a"]));
    const diamond = screen.getByRole("button", { name: "Opacity keyframe at 3.00 seconds, value 0.2" });
    // (1 s clip start + 3 s) at 80 px/s; value 0.2 sits 80% down the 24 px inner lane.
    expect(diamond).toHaveStyle({ left: "320px", top: `${8 + 0.8 * 24}px` });
    act(() => diamond.focus());
    expect(diamond).toHaveFocus();
  });

  it("commits moveItemKeyframe with the new time when a diamond is dragged, and the value when dragged vertically", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a"]));
    const diamond = screen.getByRole("button", { name: /keyframe at 3.00 seconds/ });
    fireEvent.pointerDown(diamond, { button: 0, clientX: 438, clientY: 80 });
    fireEvent.pointerMove(window, { clientX: 478, clientY: 80 });
    flushFrames();
    expect(screen.getByRole("button", { name: /keyframe at 3.50 seconds/ })).toBeInTheDocument();
    fireEvent.pointerUp(window, { clientX: 478, clientY: 80 });
    await settle();
    expect(appliedBatches).toEqual([[{ type: "moveItemKeyframe", itemId: "a", property: "opacity", fromSeconds: 3, toSeconds: 3.5 }]]);
    expect(store.getState().history.past).toHaveLength(1);

    const moved = screen.getByRole("button", { name: /keyframe at 3.50 seconds/ });
    fireEvent.pointerDown(moved, { button: 0, clientX: 478, clientY: 80 });
    // 12 px up on a 24 px inner lane is half of the 0–1 opacity range.
    fireEvent.pointerUp(window, { clientX: 478, clientY: 68 });
    await settle();
    expect(appliedBatches[1]).toEqual([
      { type: "upsertItemKeyframe", itemId: "a", property: "opacity", keyframe: { atSeconds: 3.5, value: 0.7, easing: "easeIn" } },
    ]);
  });

  it("deletes a focused diamond with Delete and moves it with the arrow keys, keeping focus", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a"]));
    const diamond = screen.getByRole("button", { name: /keyframe at 3.00 seconds/ });
    act(() => diamond.focus());
    fireEvent.keyDown(diamond, { key: "ArrowRight" });
    await settle();
    expect(appliedBatches[0]).toEqual([{ type: "moveItemKeyframe", itemId: "a", property: "opacity", fromSeconds: 3, toSeconds: 3.1 }]);
    const moved = screen.getByRole("button", { name: /keyframe at 3.10 seconds/ });
    expect(moved).toHaveFocus();

    fireEvent.keyDown(moved, { key: "Delete" });
    await settle();
    expect(appliedBatches[1]).toEqual([{ type: "deleteItemKeyframe", itemId: "a", property: "opacity", atSeconds: 3.1 }]);
    expect(screen.queryByRole("button", { name: /keyframe at 3.10 seconds/ })).not.toBeInTheDocument();
  });

  it("adds a keyframe at the pointer on double-click with the curve's value", async () => {
    const { store } = renderPanel();
    act(() => store.getState().selectItems(["a"]));
    const surface = lane();
    if (!surface) throw new Error("expected the lane");
    stubRect(surface, { left: 118, width: 2400, height: 40 });
    // 2 s into the clip, halfway between 1.0 at 1 s and 0.2 at 3 s.
    fireEvent.doubleClick(surface, { clientX: 118 + 240, clientY: 20 });
    await settle();
    expect(appliedBatches).toEqual([
      [{ type: "upsertItemKeyframe", itemId: "a", property: "opacity", keyframe: { atSeconds: 2, value: 0.6, easing: "linear" } }],
    ]);
    expect(screen.getByRole("button", { name: "Opacity keyframe at 2.00 seconds, value 0.6" })).toHaveFocus();
  });
});
