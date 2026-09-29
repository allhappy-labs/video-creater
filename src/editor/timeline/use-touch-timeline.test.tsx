import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { installPointerEventPolyfill, renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { TimelinePanel } from "./timeline-panel";
import { crossfade, transitionTestProject } from "./transition-test-project";
import { longPressMilliseconds, longPressTolerancePixels } from "../shell/use-long-press";
import { centeredPlayheadSeconds, scrollLeftForCenteredPlayhead } from "./use-touch-timeline";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function clip(id: string, startSeconds: number, durationSeconds: number): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {} };
}

/** One 30 s video track with "a" at 0–2 s and "b" at 4–6 s; jsdom lanes are 720 px wide, so padding is 360 px. */
function projectWith(): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: [{ id: "v1", name: "v1", kind: "video", locked: false, enabled: true, items: [clip("a", 0, 2), clip("b", 4, 2)] }],
  };
  return project;
}

function renderMobilePanel() {
  return renderWithEditorStore(<TimelinePanel mobile />, { project: projectWith() });
}

function scroller(): HTMLElement {
  return screen.getByTestId("timeline-scroller");
}

function option(itemId: string): HTMLElement {
  const element = screen.getAllByRole("option").find((candidate) => candidate.dataset.itemId === itemId);
  if (!element) throw new Error(`no option for ${itemId}`);
  return element;
}

function touchDown(target: Element, pointerId: number, clientX: number, clientY = 20) {
  fireEvent.pointerDown(target, { button: 0, pointerId, pointerType: "touch", clientX, clientY });
}

describe("centred playhead maths", () => {
  const geometry = { pixelsPerSecond: 80, viewportWidth: 400, centerPadding: 200 };

  it("maps scroll offsets to the time under the centre and back", () => {
    expect(centeredPlayheadSeconds(geometry, 0)).toBe(0);
    expect(centeredPlayheadSeconds(geometry, 160)).toBe(2);
    expect(scrollLeftForCenteredPlayhead(geometry, 2)).toBe(160);
  });
});

describe("useTouchTimeline", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  });

  afterEach(() => vi.useRealTimers());

  it("seeks to the time under the centre when the content scrolls", () => {
    const { store } = renderMobilePanel();
    const element = scroller();
    element.scrollLeft = 160;
    fireEvent.scroll(element);
    expect(store.getState().playheadSeconds).toBe(2);
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: "360px" });
  });

  it("scrolls the content when playback or a seek moves the playhead", () => {
    const { store } = renderMobilePanel();
    act(() => store.getState().seek(3));
    expect(store.getState().scrollLeft).toBe(240);
    expect(scroller().scrollLeft).toBe(240);
  });

  it("follows the playhead during playback until a manual scroll, then resumes when playback restarts", () => {
    const { store } = renderMobilePanel();
    act(() => store.getState().togglePlaying());
    act(() => store.getState().seek(3));
    expect(scroller().scrollLeft).toBe(240);

    // The finger scrolls ahead: playback keeps its time and the view stops following.
    const element = scroller();
    element.scrollLeft = 400;
    fireEvent.scroll(element);
    expect(store.getState().playheadSeconds).toBe(3);
    act(() => store.getState().seek(4));
    expect(store.getState().scrollLeft).toBe(400);
    // The playhead leaves the centre (360 px) and sits at its time: 4 s × 80 px/s + 360 px padding − 400 px scroll.
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: "280px" });

    // Stopping and restarting playback resumes the follow.
    act(() => store.getState().togglePlaying());
    expect(store.getState().scrollLeft).toBe(320);
    act(() => store.getState().togglePlaying());
    act(() => store.getState().seek(5));
    expect(store.getState().scrollLeft).toBe(400);
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: "360px" });
  });

  it("does not treat follow echoes during playback as manual scrolls", () => {
    const { store } = renderMobilePanel();
    act(() => store.getState().togglePlaying());
    act(() => store.getState().seek(2));
    fireEvent.scroll(scroller());
    act(() => store.getState().seek(3));
    expect(store.getState().scrollLeft).toBe(240);
  });

  it("keeps the playhead time under the centre when the zoom changes", () => {
    const { store } = renderMobilePanel();
    act(() => store.getState().seek(3));
    act(() => store.getState().setZoomPercent(200));
    expect(store.getState().scrollLeft).toBe(480);
    expect(store.getState().playheadSeconds).toBe(3);
  });

  it("pinches the zoom with two touch pointers", () => {
    const { store } = renderMobilePanel();
    const lane = screen.getByRole("listbox", { name: "Video 1" });
    touchDown(lane, 1, 100);
    touchDown(lane, 2, 200);
    fireEvent.pointerMove(lane, { pointerId: 2, pointerType: "touch", clientX: 300, clientY: 20 });
    expect(store.getState().zoomPercent).toBe(200);
    fireEvent.pointerMove(lane, { pointerId: 2, pointerType: "touch", clientX: 150, clientY: 20 });
    expect(store.getState().zoomPercent).toBe(50);
    fireEvent.pointerUp(lane, { pointerId: 2, pointerType: "touch", clientX: 150, clientY: 20 });
    fireEvent.pointerMove(lane, { pointerId: 1, pointerType: "touch", clientX: 40, clientY: 20 });
    expect(store.getState().zoomPercent).toBe(50);
  });

  it("does not select a clip under the second finger of a pinch", () => {
    const { store } = renderMobilePanel();
    touchDown(screen.getByRole("listbox", { name: "Video 1" }), 1, 700);
    touchDown(option("b"), 2, 400);
    expect(store.getState().selectedItemIds).toEqual([]);
  });

  it("opens the clip context menu after a 500 ms long press", () => {
    renderMobilePanel();
    touchDown(option("b"), 1, 400);
    fireEvent.pointerMove(option("b"), { pointerId: 1, pointerType: "touch", clientX: 400 + longPressTolerancePixels, clientY: 20 });
    act(() => vi.advanceTimersByTime(longPressMilliseconds - 1));
    expect(screen.queryByRole("menu", { name: "Clip actions" })).not.toBeInTheDocument();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.getByRole("menu", { name: "Clip actions" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /Split at playhead/ })).toBeInTheDocument();
  });

  it("opens the transition menu on a long-pressed cut badge and the track menu on an empty lane", () => {
    const { unmount } = renderWithEditorStore(<TimelinePanel mobile />, { project: transitionTestProject(undefined, [crossfade()]) });
    touchDown(screen.getByRole("button", { name: "Crossfade transition, 0.5s" }), 1, 400);
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(screen.getByRole("menu", { name: "Transition actions" })).toBeInTheDocument();
    unmount();

    renderMobilePanel();
    touchDown(screen.getByRole("listbox", { name: "Video 1" }), 1, 700);
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(screen.getByRole("menu", { name: "Track actions" })).toBeInTheDocument();
  });

  it("cancels the long press when the finger moves more than 8 px", () => {
    renderMobilePanel();
    touchDown(option("b"), 1, 400);
    fireEvent.pointerMove(option("b"), { pointerId: 1, pointerType: "touch", clientX: 400 + longPressTolerancePixels + 1, clientY: 20 });
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("ignores mouse presses", () => {
    renderMobilePanel();
    fireEvent.pointerDown(option("b"), { button: 0, pointerId: 1, pointerType: "mouse", clientX: 400, clientY: 20 });
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
