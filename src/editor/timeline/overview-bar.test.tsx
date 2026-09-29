import "@testing-library/jest-dom/vitest";
import { fireEvent, screen } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { installPointerEventPolyfill, renderWithEditorStore, stubRect } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { OverviewBar } from "./overview-bar";
import { computeTimelineGeometry } from "./use-timeline-geometry";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderOverview() {
  const project = fixtureProject();
  project.timeline.durationSeconds = 60;
  // 60 s at 100% is 4800 px plus 160 px trailing space: 62 rendered seconds, 9 of them visible.
  const geometry = computeTimelineGeometry({ timeline: project.timeline, zoomPercent: 100, scrollLeft: 0, viewportWidth: 720, mobile: false });
  const rendered = renderWithEditorStore(<OverviewBar geometry={geometry} />, { project });
  const bar = screen.getByRole("scrollbar", { name: "Timeline overview" });
  stubRect(bar, { left: 0, width: 400, height: 18 });
  return { geometry, bar, ...rendered };
}

describe("OverviewBar", () => {
  beforeAll(installPointerEventPolyfill);
  beforeEach(() => window.localStorage.clear());

  it("draws one density mark per clip in its kind color", () => {
    const { container, geometry } = renderOverview();
    const marks = container.querySelectorAll("[data-overview-mark]");
    const itemCount = geometry.rows.reduce((total, row) => total + row.track.items.length, 0);
    expect(marks).toHaveLength(itemCount);
    expect(container.querySelector('[data-overview-mark="music-bed"]')).toHaveClass("bg-clip-audio");
    expect(container.querySelector('[data-overview-mark="caption-1"]')).toHaveClass("bg-clip-caption");
  });

  it("sizes the window to the visible fraction of the content", () => {
    renderOverview();
    const window = screen.getByTestId("overview-window");
    expect(window.style.left).toBe("0%");
    expect(Number.parseFloat(window.style.width)).toBeCloseTo((720 / 4960) * 100, 3);
  });

  it("pans the view when the window is dragged", () => {
    const { store, bar } = renderOverview();
    fireEvent.pointerDown(screen.getByTestId("overview-window"), { clientX: 100, pointerId: 1, button: 0 });
    fireEvent.pointerMove(bar, { clientX: 200, pointerId: 1 });
    // 100 px of a 400 px bar is a quarter of 62 rendered seconds.
    expect(store.getState().scrollLeft).toBeCloseTo(15.5 * 80, 3);
    fireEvent.pointerUp(bar, { clientX: 200, pointerId: 1 });
    fireEvent.pointerMove(bar, { clientX: 300, pointerId: 1 });
    expect(store.getState().scrollLeft).toBeCloseTo(15.5 * 80, 3);
  });

  it("zooms when a window edge is dragged", () => {
    const { store, bar } = renderOverview();
    fireEvent.pointerDown(screen.getByTestId("overview-window-end"), { clientX: 58, pointerId: 1, button: 0 });
    fireEvent.pointerMove(bar, { clientX: 8, pointerId: 1 });
    // The window shrinks from 0–9 s to 0–1.25 s, so 720 px now spans 1.25 s: 576 px/s = 720%.
    expect(store.getState().zoomPercent).toBeCloseTo(720, 3);
    expect(store.getState().scrollLeft).toBe(0);
  });

  it("jumps the view to a clicked point, centered", () => {
    const { store, bar } = renderOverview();
    fireEvent.pointerDown(bar, { clientX: 300, pointerId: 1, button: 0 });
    // 300/400 of 62 s is 46.5 s; centering 9 visible seconds starts the view at 42 s.
    expect(store.getState().scrollLeft).toBeCloseTo(42 * 80, 3);
  });

  it("pans with the arrow keys", () => {
    const { store, bar } = renderOverview();
    fireEvent.keyDown(bar, { key: "ArrowRight" });
    expect(store.getState().scrollLeft).toBeCloseTo(72, 3);
  });
});
