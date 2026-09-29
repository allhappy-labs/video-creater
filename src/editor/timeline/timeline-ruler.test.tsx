import "@testing-library/jest-dom/vitest";
import { fireEvent, screen } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { installPointerEventPolyfill, renderWithEditorStore, stubRect } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { TimelineRuler } from "./timeline-ruler";
import { computeTimelineGeometry } from "./use-timeline-geometry";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderRuler(zoomPercent: number) {
  const project = fixtureProject();
  const geometry = computeTimelineGeometry({ timeline: project.timeline, zoomPercent, scrollLeft: 0, viewportWidth: 720, mobile: false });
  return { geometry, ...renderWithEditorStore(<TimelineRuler geometry={geometry} />, { project }) };
}

function labels(): string[] {
  return screen.getAllByTestId("ruler-label").map((label) => label.textContent ?? "");
}

describe("TimelineRuler", () => {
  beforeAll(installPointerEventPolyfill);
  beforeEach(() => window.localStorage.clear());

  it("labels coarse ticks when zoomed out", () => {
    renderRuler(10);
    expect(labels()).toEqual([
      "00:00:00", "00:00:10", "00:00:20", "00:00:30", "00:00:40",
      "00:00:50", "00:01:00", "00:01:10", "00:01:20", "00:01:30",
    ]);
    expect(screen.queryAllByTestId("ruler-minor-tick")).toHaveLength(0);
  });

  it("labels whole seconds with quarter-second minor ticks at 100%", () => {
    renderRuler(100);
    expect(labels()).toEqual([
      "00:00:00", "00:00:01", "00:00:02", "00:00:03", "00:00:04", "00:00:05",
      "00:00:06", "00:00:07", "00:00:08", "00:00:09", "00:00:10",
    ]);
    expect(screen.getAllByTestId("ruler-minor-tick")).toHaveLength(30);
  });

  it("labels quarter seconds when zoomed in, only inside the render window", () => {
    renderRuler(1000);
    expect(labels()).toEqual([
      "00:00:00", "00:00:00.250", "00:00:00.500", "00:00:00.750",
      "00:00:01", "00:00:01.250", "00:00:01.500", "00:00:01.750",
    ]);
  });

  it("positions ticks at seconds times pixels per second", () => {
    renderRuler(100);
    const [, second] = screen.getAllByTestId("ruler-major-tick");
    expect(second).toHaveStyle({ left: "80px" });
  });

  it("seeks on pointer down and while dragging", () => {
    const { store } = renderRuler(100);
    const ruler = screen.getByTestId("timeline-ruler");
    stubRect(ruler, { left: 100, width: 800, height: 24 });

    fireEvent.pointerDown(ruler, { clientX: 260, pointerId: 1, button: 0 });
    expect(store.getState().playheadSeconds).toBe(2);
    fireEvent.pointerMove(ruler, { clientX: 340, pointerId: 1 });
    expect(store.getState().playheadSeconds).toBe(3);
    fireEvent.pointerUp(ruler, { clientX: 340, pointerId: 1 });
    fireEvent.pointerMove(ruler, { clientX: 500, pointerId: 1 });
    expect(store.getState().playheadSeconds).toBe(3);
  });

  it("clamps seeks to the timeline duration", () => {
    const { store } = renderRuler(100);
    const ruler = screen.getByTestId("timeline-ruler");
    stubRect(ruler, { left: 0, width: 800, height: 24 });
    fireEvent.pointerDown(ruler, { clientX: 790, pointerId: 1, button: 0 });
    expect(store.getState().playheadSeconds).toBe(8);
  });
});
