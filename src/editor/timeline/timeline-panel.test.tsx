import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { TimelinePanel } from "./timeline-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderPanel(mobile = false) {
  const project = fixtureProject();
  project.timeline.durationSeconds = 60;
  return renderWithEditorStore(<TimelinePanel mobile={mobile} />, { project });
}

function scroller(): HTMLElement {
  const overview = screen.getByRole("scrollbar", { name: "Timeline overview" });
  const id = overview.getAttribute("aria-controls");
  const element = id ? document.getElementById(id) : null;
  if (!element) throw new Error("overview does not control a scroll container");
  return element;
}

describe("TimelinePanel", () => {
  beforeEach(() => window.localStorage.clear());

  it("keeps the timeline landmarks and composes ruler, playhead and overview", () => {
    renderPanel();
    expect(screen.getByRole("toolbar", { name: "Timeline tools" })).toBeInTheDocument();
    const canvas = screen.getByRole("region", { name: "Timeline canvas" });
    expect(canvas).toContainElement(screen.getByTestId("timeline-ruler"));
    expect(canvas).toContainElement(screen.getByTestId("playhead"));
    expect(canvas).toContainElement(screen.getByRole("scrollbar", { name: "Timeline overview" }));
  });

  it("syncs native horizontal scrolling with the store both ways", () => {
    const { store } = renderPanel();
    const element = scroller();
    element.scrollLeft = 320;
    fireEvent.scroll(element);
    expect(store.getState().scrollLeft).toBe(320);

    act(() => store.getState().setScrollLeft(640));
    expect(element.scrollLeft).toBe(640);
  });

  it("offsets the playhead by the header column on desktop and fixes it at the centre on mobile", () => {
    const desktop = renderPanel();
    act(() => desktop.store.getState().seek(1));
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: `${118 + 80}px` });
    desktop.unmount();

    // jsdom has no layout, so the lanes viewport falls back to 720 px.
    const mobile = renderPanel(true);
    act(() => mobile.store.getState().seek(1));
    expect(screen.getByTestId("playhead")).toHaveStyle({ left: "360px" });
  });

  it("renders a named listbox and a header per track in band order", () => {
    renderPanel();
    expect(screen.getAllByRole("listbox").map((listbox) => listbox.getAttribute("aria-label"))).toEqual([
      "Graphics 1",
      "Text 1",
      "Captions",
      "Video 1",
      "Audio 1",
    ]);
    expect(screen.getAllByRole("group", { name: / track$/ })).toHaveLength(5);
    expect(screen.getByRole("option", { name: "Opening clip, 00:00:00, 00:00:04" })).toBeInTheDocument();
    expect(screen.getByRole("listbox", { name: "Video 1" })).toHaveStyle({ height: "58px" });
  });

  it("keeps every track row inside the scroller, clear of the overview bar below it", () => {
    // Flow 1 at 1440×900: a 226 px scroller holds the ruler and six rows once an insert adds a track.
    const clientHeight = vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function (this: HTMLElement) {
      return this.dataset.testid === "timeline-scroller" ? 226 : 0;
    });
    try {
      const project = fixtureProject();
      const video = project.timeline.tracks.find((track) => track.kind === "video");
      if (!video) throw new Error("sample has no video track");
      project.timeline.tracks.push({ ...video, id: "video-2", name: "Video 2", items: [] });
      renderWithEditorStore(<TimelinePanel />, { project });

      const overview = screen.getByRole("scrollbar", { name: "Timeline overview" });
      const element = scroller();
      // The bar is a fixed-height sibling after the flexible scroll area, never inside or over it.
      expect(element).not.toContainElement(overview);
      expect(element).toHaveClass("absolute", "inset-0", "overflow-auto");
      expect(element.parentElement).toHaveClass("relative", "min-h-0", "flex-1");
      const barRow = overview.parentElement;
      expect(barRow).toHaveClass("shrink-0");
      expect(element.parentElement?.compareDocumentPosition(barRow as Node) ?? 0).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

      const heights = screen.getAllByRole("listbox").map((listbox) => Number.parseFloat(listbox.style.height));
      expect(heights).toHaveLength(6);
      expect(24 + heights.reduce((sum, height) => sum + height, 0)).toBeLessThanOrEqual(226);
    } finally {
      clientHeight.mockRestore();
    }
  });

  it("hides track headers, the toolbar and the overview on mobile", () => {
    renderPanel(true);
    expect(screen.getAllByRole("listbox")).toHaveLength(5);
    expect(screen.queryByRole("group", { name: / track$/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Hide Video 1" })).not.toBeInTheDocument();
    expect(screen.queryByRole("toolbar", { name: "Timeline tools" })).not.toBeInTheDocument();
    expect(screen.queryByRole("scrollbar", { name: "Timeline overview" })).not.toBeInTheDocument();
  });

  it("pads the mobile lanes by half the viewport so time 0 reaches the centre", () => {
    renderPanel(true);
    const content = screen.getByTestId("timeline-scroller").firstElementChild;
    // 60 s at 80 px/s plus 360 px on each side.
    expect(content).toHaveStyle({ width: `${4800 + 720}px`, paddingLeft: "360px", paddingRight: "360px" });
  });

  it("opens the Media sheet from the + at the end of the main track on mobile", () => {
    const { store } = renderPanel(true);
    fireEvent.click(screen.getByRole("button", { name: "Add media" }));
    expect(store.getState().openSheetId).toBe("media");
  });

  it("has no + button on desktop", () => {
    renderPanel();
    expect(screen.queryByRole("button", { name: "Add media" })).not.toBeInTheDocument();
  });

  it("windows a 500-clip project while scrolling and zooming", () => {
    const project = fixtureProject();
    // 1 s clips every 2 s: "c0" at 0 s through "c499" at 998 s.
    const items = Array.from({ length: 500 }, (_, index) => ({
      id: `c${index.toString()}`,
      kind: "video_clip" as const,
      startSeconds: index * 2,
      durationSeconds: 1,
      source: { type: "media" as const, mediaId: "media-1" },
      label: `c${index.toString()}`,
      properties: {},
    }));
    project.timeline = { durationSeconds: 1000, tracks: [{ id: "v1", name: "v1", kind: "video", locked: false, enabled: true, items }] };
    const { store } = renderWithEditorStore(<TimelinePanel />, { project });
    // The jsdom lanes viewport is 720 px; one viewport of overscan on each side renders three.
    const renderedIds = () => screen.getAllByRole("option").map((option) => option.dataset.itemId);
    const bound = (pixelsPerSecond: number) => Math.ceil((3 * 720) / pixelsPerSecond / 2) + 1;

    expect(renderedIds().length).toBeLessThanOrEqual(bound(80));
    expect(renderedIds()).toContain("c0");

    act(() => store.getState().setScrollLeft(80 * 500));
    expect(renderedIds().length).toBeLessThanOrEqual(bound(80));
    expect(renderedIds()).toContain("c250");
    expect(renderedIds()).not.toContain("c0");

    act(() => {
      store.getState().setZoomPercent(25);
      store.getState().setScrollLeft(20 * 400);
    });
    expect(renderedIds().length).toBeLessThanOrEqual(bound(20));
    expect(renderedIds()).toContain("c200");
    expect(renderedIds()).not.toContain("c0");
    expect(renderedIds()).not.toContain("c499");
  });

  it("shows blocked-command feedback politely and dismisses it", () => {
    const { store } = renderPanel();
    const status = screen.getByRole("status");
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(status).toBeEmptyDOMElement();

    act(() => store.getState().setLastError("Move the playhead over the selected clip to split."));
    expect(screen.getByRole("status")).toHaveTextContent("Move the playhead over the selected clip to split.");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss message" }));
    expect(store.getState().lastError).toBeNull();
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    expect(screen.queryByRole("button", { name: "Dismiss message" })).not.toBeInTheDocument();
  });

  it("returns the preview from asset preview on any timeline pointer interaction", () => {
    const { store } = renderPanel();
    act(() => store.getState().previewAsset("media-1"));
    fireEvent.pointerDown(screen.getByRole("region", { name: "Timeline canvas" }));
    expect(store.getState().previewSource).toEqual({ kind: "timeline" });
  });
});
