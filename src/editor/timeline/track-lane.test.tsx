import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { requiredValue } from "@/test-utils/required";
import { TrackLane } from "./track-lane";
import { computeTimelineGeometry } from "./use-timeline-geometry";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function clip(id: string, startSeconds: number, durationSeconds = 2): TimelineItem {
  return {
    id,
    kind: "video_clip",
    startSeconds,
    durationSeconds,
    source: { type: "media", mediaId: "media-1" },
    label: id,
    properties: {},
  };
}

function projectWith(items: TimelineItem[], patch: Partial<TimelineTrack> = {}): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 120,
    tracks: [{ id: "v1", name: "Main", kind: "video", locked: false, enabled: true, items, ...patch }],
  };
  return project;
}

/** Renders the lane from the store project so selection changes re-render it. */
function renderLane(project: VideoProject, scrollLeft = 0, touch = false) {
  const geometry = computeTimelineGeometry({ timeline: project.timeline, zoomPercent: 100, scrollLeft, viewportWidth: 720, mobile: false });
  const row = requiredValue(geometry.rows[0], "first row");
  return renderWithEditorStore(<TrackLane row={row} geometry={geometry} touch={touch} />, { project, projectDir: "browser://bundled-sample-project" });
}

function optionNames(): string[] {
  return screen.getAllByRole("option").map((option) => option.getAttribute("aria-label") ?? "");
}

describe("TrackLane", () => {
  beforeEach(() => window.localStorage.clear());

  it("is a listbox named by the track display name with one option per clip", () => {
    renderLane(projectWith([clip("b", 4), clip("a", 0, 1.5)]));
    const listbox = screen.getByRole("listbox", { name: "Video 1" });
    expect(listbox).toHaveAttribute("aria-multiselectable", "true");
    expect(within(listbox).getAllByRole("option").map((option) => option.getAttribute("aria-label"))).toEqual([
      "a, 00:00:00, 00:00:01.500",
      "b, 00:00:04, 00:00:02",
    ]);
    expect(listbox).toHaveStyle({ height: "58px" });
  });

  it("renders only clips inside the render window, plus selected clips", () => {
    // 720 px at 80 px/s shows 9 s; one viewport of overscan renders 0–18 s.
    const { store } = renderLane(projectWith([clip("near", 2), clip("edge", 17), clip("far", 60), clip("farther", 90)]));
    expect(optionNames().map((name) => name.split(",")[0])).toEqual(["near", "edge"]);
    act(() => store.getState().selectItems(["farther"]));
    expect(optionNames().map((name) => name.split(",")[0])).toEqual(["near", "edge", "farther"]);
  });

  it("keeps a single tab stop, on the selected clip when there is one", () => {
    const { store } = renderLane(projectWith([clip("a", 0), clip("b", 3), clip("c", 6)]));
    const tabStops = () => screen.getAllByRole("option").filter((option) => option.tabIndex === 0);
    expect(tabStops().map((option) => option.dataset.itemId)).toEqual(["a"]);
    act(() => store.getState().selectItems(["b"]));
    expect(tabStops().map((option) => option.dataset.itemId)).toEqual(["b"]);
  });

  it("moves focus with Arrow Left and Right and selects with Enter", () => {
    const { store } = renderLane(projectWith([clip("a", 0), clip("b", 3), clip("c", 6)]));
    const [first] = screen.getAllByRole("option");
    act(() => first?.focus());

    fireEvent.keyDown(document.activeElement ?? document.body, { key: "ArrowRight" });
    expect(document.activeElement).toHaveAttribute("data-item-id", "b");
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "ArrowRight" });
    expect(document.activeElement).toHaveAttribute("data-item-id", "c");
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "ArrowRight" });
    expect(document.activeElement).toHaveAttribute("data-item-id", "c");
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "ArrowLeft" });
    expect(document.activeElement).toHaveAttribute("data-item-id", "b");

    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Enter" });
    expect(store.getState().selectedItemIds).toEqual(["b"]);
    expect(document.activeElement).toHaveAttribute("aria-selected", "true");

    fireEvent.keyDown(document.activeElement ?? document.body, { key: "ArrowLeft" });
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Enter", shiftKey: true });
    expect(store.getState().selectedItemIds).toEqual(["b", "a"]);
  });

  it("renders and focuses a clip outside the render window when arrowing to it", () => {
    renderLane(projectWith([clip("a", 0), clip("far", 80)]));
    expect(optionNames()).toHaveLength(1);
    act(() => screen.getByRole("option").focus());
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "ArrowRight" });
    expect(document.activeElement).toHaveAttribute("data-item-id", "far");
  });

  it("selects on click, toggling with a modifier", () => {
    const { store } = renderLane(projectWith([clip("a", 0), clip("b", 3)]));
    const [first, second] = screen.getAllByRole("option");
    if (!first || !second) throw new Error("expected two options");
    fireEvent.click(first);
    fireEvent.click(second, { metaKey: true });
    expect(store.getState().selectedItemIds).toEqual(["a", "b"]);
    fireEvent.click(first, { ctrlKey: true });
    expect(store.getState().selectedItemIds).toEqual(["b"]);
  });

  it("renders trim handles beside the listbox for selected clips, widened on touch and hidden on locked tracks", () => {
    const desktop = renderLane(projectWith([clip("a", 1), clip("b", 4)]));
    expect(screen.queryAllByRole("slider")).toHaveLength(0);
    act(() => desktop.store.getState().selectItems(["a"]));
    const handles = screen.getAllByRole("slider");
    expect(handles.map((handle) => handle.getAttribute("aria-label"))).toEqual(["Resize a left edge", "Resize a right edge"]);
    expect(within(screen.getByRole("listbox")).queryByRole("slider")).not.toBeInTheDocument();
    // 7 px bars inside the clip (80–240 px).
    expect(handles[0]).toHaveStyle({ left: "80px", width: "7px" });
    expect(handles[1]).toHaveStyle({ left: "233px", width: "7px" });
    desktop.unmount();

    const touch = renderLane(projectWith([clip("a", 1)]), 0, true);
    act(() => touch.store.getState().selectItems(["a"]));
    expect(screen.getAllByRole("slider")[1]).toHaveStyle({ left: "224px", width: "24px" });
    touch.unmount();

    const locked = renderLane(projectWith([clip("a", 1)], { locked: true }));
    act(() => locked.store.getState().selectItems(["a"]));
    expect(screen.queryAllByRole("slider")).toHaveLength(0);
  });

  it("dims clips on a hidden track", () => {
    renderLane(projectWith([clip("a", 0)], { enabled: false }));
    expect(screen.getByTestId("clip-body")).toHaveClass("opacity-45");
  });
});
