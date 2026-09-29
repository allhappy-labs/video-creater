import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { TimelineItem } from "@/lib/timeline";
import { installPointerEventPolyfill, renderWithEditorStore, stubRect } from "@/test-utils/editor-render";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (p: string) => p,
}));

const { PreviewPanel } = await import("./preview-panel");
const { PropertiesPanel } = await import("../properties/properties-panel");
const { installMediaAndFrameStubs } = await import("./media-element-stubs");

/** The canvas is 960 x 540 CSS pixels for a 1920 x 1080 render. */
function renderCanvas(project: VideoProject = fixtureProject()) {
  const result = renderWithEditorStore(<PreviewPanel />, { project, projectDir: "/p" });
  stubRect(surface(), { width: 960, height: 540 });
  return result;
}

function surface() {
  return screen.getByTestId("preview-canvas-surface");
}

function video(label = "Opening clip") {
  return within(screen.getByRole("region", { name: "Preview viewport" })).getByLabelText(`Timeline video ${label}`);
}

function openingItem(project: VideoProject): TimelineItem {
  return fixtureItem(project, "video");
}

/** Opening clip as a half-size box in the middle of the canvas. */
function halfSizeProject() {
  const project = fixtureProject();
  openingItem(project).properties.transform = { centerX: 0.5, centerY: 0.5, width: 0.5, height: 0.5 };
  return project;
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

function drag(target: Element, from: readonly [number, number], to: readonly [number, number], init: PointerEventInit = {}) {
  fireEvent.pointerDown(target, { button: 0, pointerId: 1, clientX: from[0], clientY: from[1], ...init });
  fireEvent.pointerMove(surface(), { pointerId: 1, clientX: to[0], clientY: to[1], ...init });
  fireEvent.pointerUp(surface(), { pointerId: 1, clientX: to[0], clientY: to[1], ...init });
}

describe("preview canvas selection and transform", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("selects the topmost layer under a click and clears the selection on empty canvas", () => {
    const project = halfSizeProject();
    project.timeline.tracks.splice(1, 0, {
      ...fixtureTrack(project, "video"),
      id: "track-top",
      name: "Top",
      items: [
        {
          ...openingItem(project),
          id: "top-clip",
          label: "Top clip",
          // Moved 480 of 1920 output pixels right: centered at x = 0.75 on the canvas.
          properties: { transform: { centerX: 0.5, centerY: 0.5, width: 0.2, height: 0.2 }, positionX: 480 },
        },
      ],
    });
    const { store } = renderCanvas(project);

    fireEvent.pointerDown(surface(), { button: 0, pointerId: 1, clientX: 720, clientY: 270 });
    fireEvent.pointerUp(surface(), { pointerId: 1, clientX: 720, clientY: 270 });
    expect(store.getState().selectedItemIds).toEqual(["top-clip"]);
    expect(screen.getByRole("group", { name: "Canvas transform controls for Top clip" })).toBeInTheDocument();

    fireEvent.pointerDown(surface(), { button: 0, pointerId: 1, clientX: 400, clientY: 270 });
    fireEvent.pointerUp(surface(), { pointerId: 1, clientX: 400, clientY: 270 });
    expect(store.getState().selectedItemIds).toEqual(["item-1"]);

    fireEvent.pointerDown(surface(), { button: 0, pointerId: 1, clientX: 20, clientY: 20 });
    expect(store.getState().selectedItemIds).toEqual([]);
    expect(screen.queryByRole("group", { name: /Canvas transform controls/ })).not.toBeInTheDocument();
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("commits one transform for a corner scale drag and shows it while dragging", async () => {
    const { store } = renderCanvas(halfSizeProject());
    act(() => store.getState().selectItems(["item-1"]));
    const corner = screen.getByRole("button", { name: "Resize Opening clip from bottom right" });

    fireEvent.pointerDown(corner, { button: 0, pointerId: 1, clientX: 720, clientY: 405 });
    fireEvent.pointerMove(surface(), { pointerId: 1, clientX: 768, clientY: 432 });
    expect(video().style.width).toBe("55%");
    expect(store.getState().history.past).toHaveLength(0);
    fireEvent.pointerUp(surface(), { pointerId: 1, clientX: 768, clientY: 432 });
    await settle();

    expect(store.getState().history.past).toHaveLength(1);
    expect(openingItem(store.getState().project).properties.transform).toEqual({ centerX: 0.525, centerY: 0.525, width: 0.55, height: 0.55 });
    expect(video().style.width).toBe("55%");
  });

  it("does not commit a press that moves less than 2 px", async () => {
    const { store } = renderCanvas(halfSizeProject());
    drag(surface(), [480, 270], [481, 271]);
    await settle();
    expect(store.getState().selectedItemIds).toEqual(["item-1"]);
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("moves with a drag and with arrow keys, one undo step each", async () => {
    const { store } = renderCanvas(halfSizeProject());
    drag(surface(), [480, 270], [576, 324]);
    await settle();
    expect(openingItem(store.getState().project).properties.transform).toMatchObject({ centerX: 0.6, centerY: 0.6 });

    fireEvent.keyDown(screen.getByRole("button", { name: "Move Opening clip in preview canvas" }), { key: "ArrowLeft" });
    await settle();
    expect(openingItem(store.getState().project).properties.transform).toMatchObject({ centerX: 0.59, centerY: 0.6 });
    fireEvent.keyDown(screen.getByRole("button", { name: "Resize Opening clip from top left" }), { key: "ArrowLeft" });
    await settle();
    expect(openingItem(store.getState().project).properties.transform).toMatchObject({ width: 0.51, centerX: 0.585 });
    expect(store.getState().history.past).toHaveLength(3);
    expect(store.getState().playheadSeconds).toBe(0);
  });

  it("rotates with a Shift drag snapped to 15 degrees and with arrow keys", async () => {
    const { store } = renderCanvas(halfSizeProject());
    act(() => store.getState().selectItems(["item-1"]));
    const rotate = screen.getByRole("button", { name: "Rotate Opening clip in preview canvas" });
    expect(rotate).toHaveAttribute("title", "Drag to rotate. Hold Shift to snap to 15 degrees.");
    // From straight right of the center (480, 270) to 26 degrees below it.
    drag(rotate, [580, 270], [480 + 100 * Math.cos(0.4538), 270 + 100 * Math.sin(0.4538)], { shiftKey: true });
    await settle();
    expect(openingItem(store.getState().project).properties.rotationDegrees).toBe(30);

    fireEvent.keyDown(screen.getByRole("button", { name: "Rotate Opening clip in preview canvas" }), { key: "ArrowRight", shiftKey: true });
    await settle();
    expect(openingItem(store.getState().project).properties.rotationDegrees).toBe(45);
    expect(store.getState().history.past).toHaveLength(2);
  });

  it("upserts a rotation keyframe at the playhead when rotation is keyframed", async () => {
    const project = halfSizeProject();
    openingItem(project).properties.keyframes = { rotationDegrees: [{ atSeconds: 0, value: 0, easing: "linear" }] };
    const { store } = renderCanvas(project);
    act(() => store.getState().seek(1));
    act(() => store.getState().selectItems(["item-1"]));
    fireEvent.keyDown(screen.getByRole("button", { name: "Rotate Opening clip in preview canvas" }), { key: "ArrowLeft" });
    await settle();
    const item = openingItem(store.getState().project);
    expect(item.properties.rotationDegrees).toBeUndefined();
    expect(item.properties.keyframes).toMatchObject({ rotationDegrees: [{ atSeconds: 0 }, { atSeconds: 1, value: -1 }] });
  });

  it("ignores locked tracks", () => {
    const project = halfSizeProject();
    fixtureTrack(project, "video").locked = true;
    const { store } = renderCanvas(project);
    act(() => store.getState().selectItems(["item-1"]));
    expect(screen.queryByRole("group", { name: /Canvas transform controls/ })).not.toBeInTheDocument();
  });
});

describe("preview canvas crop mode", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    installMediaAndFrameStubs();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  function enterCropByDoubleClick() {
    const result = renderCanvas(halfSizeProject());
    fireEvent.pointerDown(surface(), { button: 0, pointerId: 1, clientX: 480, clientY: 270 });
    fireEvent.pointerUp(surface(), { pointerId: 1, clientX: 480, clientY: 270 });
    fireEvent.doubleClick(screen.getByRole("button", { name: "Move Opening clip in preview canvas" }));
    return result;
  }

  it("enters crop mode on double-click and commits the dragged crop on Enter", async () => {
    const { store } = enterCropByDoubleClick();
    expect(store.getState().cropModeItemId).toBe("item-1");
    const group = screen.getByRole("group", { name: "Canvas crop controls for Opening clip" });
    expect(screen.queryByRole("group", { name: /Canvas transform controls/ })).not.toBeInTheDocument();
    expect(within(group).getByRole("button", { name: "Crop Opening clip from top" })).toHaveFocus();

    // The layer is 480 px wide, so 48 px is a tenth of it.
    drag(within(group).getByRole("button", { name: "Crop Opening clip from left" }), [240, 270], [288, 270]);
    expect(video().style.clipPath).toBe("inset(0% 0% 0% 10%)");
    fireEvent.keyDown(within(group).getByRole("button", { name: "Crop Opening clip from right" }), { key: "ArrowLeft", shiftKey: true });
    expect(video().style.clipPath).toBe("inset(0% 5% 0% 10%)");
    await settle();
    expect(store.getState().history.past).toHaveLength(0);

    fireEvent.keyDown(within(group).getByRole("button", { name: "Crop Opening clip from right" }), { key: "Enter" });
    await settle();
    expect(store.getState().history.past).toHaveLength(1);
    expect(openingItem(store.getState().project).properties).toMatchObject({ cropLeft: 0.1, cropRight: 0.05 });
    expect(store.getState().cropModeItemId).toBeNull();
    expect(screen.getByRole("group", { name: "Canvas transform controls for Opening clip" })).toBeInTheDocument();
  });

  it("cancels the crop draft on Escape", async () => {
    const { store } = enterCropByDoubleClick();
    const top = screen.getByRole("button", { name: "Crop Opening clip from top" });
    fireEvent.keyDown(top, { key: "ArrowDown" });
    expect(video().style.clipPath).toBe("inset(1% 0% 0% 0%)");
    fireEvent.keyDown(top, { key: "Escape" });
    await settle();
    expect(store.getState().cropModeItemId).toBeNull();
    expect(store.getState().selectedItemIds).toEqual(["item-1"]);
    expect(store.getState().history.past).toHaveLength(0);
    expect(video().style.clipPath).toBe("");
  });

  it("commits on a click outside the crop controls, entered from Properties", async () => {
    const { store } = renderCanvas(halfSizeProject());
    act(() => {
      store.getState().selectItems(["item-1"]);
      store.getState().setCropModeItemId("item-1");
    });
    fireEvent.keyDown(screen.getByRole("button", { name: "Crop Opening clip from bottom" }), { key: "ArrowUp" });
    fireEvent.pointerDown(document.body, { button: 0, pointerId: 2 });
    await settle();
    expect(store.getState().cropModeItemId).toBeNull();
    expect(openingItem(store.getState().project).properties.cropBottom).toBe(0.01);
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("seeks into the clip when Properties enters crop mode with the playhead outside it", async () => {
    const project = halfSizeProject();
    const { store } = renderWithEditorStore(
      <>
        <PreviewPanel />
        <PropertiesPanel />
      </>,
      { project, projectDir: "/p" },
    );
    stubRect(surface(), { width: 960, height: 540 });
    act(() => {
      store.getState().selectItems(["item-1"]);
      store.getState().seek(6);
      store.getState().setPlaying(true);
    });
    await settle();
    fireEvent.click(screen.getByRole("button", { name: "Edit on canvas" }));
    await settle();

    // Opening clip spans 0–4 s at 24 fps, so crop mode starts half a frame into it.
    expect(store.getState().playheadSeconds).toBeCloseTo(1 / 48);
    expect(store.getState().playing).toBe(false);
    expect(store.getState().cropModeItemId).toBe("item-1");
    expect(screen.getByRole("group", { name: "Canvas crop controls for Opening clip" })).toBeInTheDocument();
  });

  it("leaves crop mode without an undo step when nothing changed", async () => {
    const { store } = enterCropByDoubleClick();
    fireEvent.pointerDown(surface(), { button: 0, pointerId: 2, clientX: 20, clientY: 20 });
    await settle();
    expect(store.getState().cropModeItemId).toBeNull();
    // The outside click only ends crop mode; it does not also clear the selection.
    expect(store.getState().selectedItemIds).toEqual(["item-1"]);
    expect(store.getState().history.past).toHaveLength(0);
  });
});
