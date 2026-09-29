import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeDataTransfer, installDragEventPolyfill } from "@/test-utils/data-transfer";
import { installPointerEventPolyfill, renderWithEditorStore } from "@/test-utils/editor-render";
import { PropertiesPanel } from "../properties/properties-panel";
import { TimelinePanel } from "./timeline-panel";
import { transitionDragMimeType, writeTransitionDragData, readTransitionDragData } from "./transition-drag-data";
import { crossfade, sourceClip, transitionsOf, transitionTestProject } from "./transition-test-project";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("../properties/use-effect-catalog", () => ({ useEffectCatalog: () => ({ status: "ready", effects: [] }) }));

function renderEditor(project = transitionTestProject(undefined, [crossfade()])) {
  return renderWithEditorStore(
    <>
      <TimelinePanel />
      <PropertiesPanel />
    </>,
    { project },
  );
}

function rows(): HTMLElement {
  const element = screen.getByRole("listbox", { name: "Video 1" }).parentElement?.parentElement?.parentElement;
  if (!element) throw new Error("no rows container");
  return element;
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe("transition drag data", () => {
  it("round-trips a kind and rejects malformed payloads", () => {
    const dataTransfer = fakeDataTransfer();
    writeTransitionDragData(dataTransfer, "wipe");
    expect(readTransitionDragData(dataTransfer)).toBe("wipe");
    expect(readTransitionDragData(fakeDataTransfer({ [transitionDragMimeType]: JSON.stringify({ kind: "spin" }) }))).toBeNull();
    expect(readTransitionDragData(fakeDataTransfer({ [transitionDragMimeType]: "{" }))).toBeNull();
  });
});

describe("timeline transitions", () => {
  beforeAll(() => {
    installDragEventPolyfill();
    installPointerEventPolyfill();
  });
  beforeEach(() => window.localStorage.clear());

  it("adds exactly one transition when a tile drops within 12 px of a cut", async () => {
    const { store } = renderEditor(transitionTestProject());
    const dataTransfer = fakeDataTransfer({ [transitionDragMimeType]: JSON.stringify({ kind: "crossfade" }) });
    // The cut at 2 s is at x = 118 + 160; 10 px right of it still targets it.
    fireEvent.dragOver(rows(), { dataTransfer, clientX: 288, clientY: 29 });
    expect(dataTransfer.dropEffect).toBe("copy");
    expect(screen.getByTestId("transition-drop-target")).toBeInTheDocument();
    fireEvent.drop(rows(), { dataTransfer, clientX: 288, clientY: 29 });
    await settle();
    expect(transitionsOf(store.getState().project)).toEqual([
      { id: "transition-a-b", leftItemId: "a", rightItemId: "b", kind: "crossfade", durationSeconds: 0.5 },
    ]);
    expect(store.getState().history.past).toHaveLength(1);
    expect(screen.queryByTestId("transition-drop-target")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Crossfade transition, 0.5s" })).toHaveAttribute("aria-pressed", "true");
  });

  it("refuses a drop away from any cut", async () => {
    const { store } = renderEditor(transitionTestProject());
    const dataTransfer = fakeDataTransfer({ [transitionDragMimeType]: JSON.stringify({ kind: "wipe" }) });
    fireEvent.dragOver(rows(), { dataTransfer, clientX: 118 + 80, clientY: 29 });
    expect(dataTransfer.dropEffect).toBe("none");
    fireEvent.drop(rows(), { dataTransfer, clientX: 118 + 80, clientY: 29 });
    await settle();
    expect(store.getState().lastError).toBe("Drop the transition on a cut between two clips.");
    expect(transitionsOf(store.getState().project)).toEqual([]);
  });

  it("selects a transition from its badge and opens the Properties Transition tab", async () => {
    const { store } = renderEditor();
    const badge = screen.getByRole("button", { name: "Crossfade transition, 0.5s" });
    expect(badge).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByTestId("transition-window")).toHaveStyle({ left: "140px", width: "40px" });
    fireEvent.click(badge);
    expect(store.getState().selectedTransitionId).toBe("fade");
    expect(badge).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("heading", { level: 2, name: "Crossfade transition" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Transition" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Max 2.0s")).toBeInTheDocument();
  });

  it("paints the transition window and edge grips under clip labels, with the badge and handles on top", () => {
    const { store } = renderEditor();
    act(() => store.getState().selectTransition("fade"));
    const zIndex = (element: Element) => Number(/(?:^|\s)z-\[?(\d+)\]?(?:\s|$)/.exec(element.className)?.[1] ?? Number.NaN);
    const windowElement = screen.getByTestId("transition-window");
    const grips = document.querySelectorAll<HTMLElement>("[data-transition-grip]");
    const handles = document.querySelectorAll<HTMLElement>("[data-transition-handle]");
    const badge = screen.getByRole("button", { name: "Crossfade transition, 0.5s" });
    const labels = screen.getAllByText(/^[ab]$/).filter((label) => label.closest('[role="option"]'));
    expect(labels).toHaveLength(2);
    expect(grips).toHaveLength(2);
    expect(handles).toHaveLength(2);

    for (const overlay of [windowElement, ...grips]) {
      expect(overlay).toHaveClass("pointer-events-none");
      expect(zIndex(overlay)).toBeLessThan(zIndex(labels[0] as HTMLElement));
    }
    for (const label of labels) expect(zIndex(label)).toBe(zIndex(labels[0] as HTMLElement));
    for (const control of [badge, ...handles]) expect(zIndex(control)).toBeGreaterThan(zIndex(labels[0] as HTMLElement));
    // The grips are not inside the handles' stacking contexts, and the handles follow the badge.
    for (const handle of handles) expect(handle.querySelector("[data-transition-grip]")).toBeNull();
    expect(windowElement.compareDocumentPosition(badge) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    for (const handle of handles) expect(badge.compareDocumentPosition(handle) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // A selected clip does not open a stacking context that would put the window over its label.
    act(() => store.getState().selectItems(["b"]));
    const selectedClip = screen.getByRole("option", { name: /^b,/ });
    expect(selectedClip).toHaveAttribute("aria-selected", "true");
    expect(Number.isNaN(zIndex(selectedClip))).toBe(true);
  });

  it("drags a badge edge to change the duration symmetrically and commits one update on release", async () => {
    const { store } = renderEditor();
    fireEvent.click(screen.getByRole("button", { name: "Crossfade transition, 0.5s" }));
    const right = document.querySelector<HTMLElement>('[data-transition-handle="right"]');
    if (!right) throw new Error("no right handle");
    fireEvent.pointerDown(right, { pointerId: 1, button: 0, clientX: 300 });
    // +20 px at 80 px/s moves the edge 0.25 s: the duration grows by twice that.
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 320 });
    expect(screen.getByTestId("transition-duration-tooltip")).toHaveTextContent("1.0s");
    expect(screen.getByRole("button", { name: "Crossfade transition, 1.0s" })).toBeInTheDocument();
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 600 });
    expect(screen.getByTestId("transition-duration-tooltip")).toHaveTextContent("2.0s · max 2.0s");
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 340 });
    expect(store.getState().history.past).toHaveLength(0);
    await act(async () => {
      fireEvent.pointerUp(window, { pointerId: 1, clientX: 340 });
      await Promise.resolve();
    });
    await settle();
    expect(transitionsOf(store.getState().project)).toMatchObject([{ id: "fade", durationSeconds: 1.5 }]);
    expect(store.getState().history.past).toHaveLength(1);
    expect(screen.queryByTestId("transition-duration-tooltip")).not.toBeInTheDocument();

    // Dragging the left edge left grows it too; Escape cancels without committing.
    const left = document.querySelector<HTMLElement>('[data-transition-handle="left"]');
    if (!left) throw new Error("no left handle");
    fireEvent.pointerDown(left, { pointerId: 2, button: 0, clientX: 200 });
    fireEvent.pointerMove(window, { pointerId: 2, clientX: 180 });
    expect(screen.getByTestId("transition-duration-tooltip")).toHaveTextContent("2.0s");
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("button", { name: "Crossfade transition, 1.5s" })).toBeInTheDocument();
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("removes the selected transition with Delete or Backspace", async () => {
    const { store } = renderEditor();
    const badge = screen.getByRole("button", { name: "Crossfade transition, 0.5s" });
    fireEvent.click(badge);
    await act(async () => {
      fireEvent.keyDown(badge, { key: "Backspace" });
      await Promise.resolve();
    });
    await settle();
    expect(transitionsOf(store.getState().project)).toEqual([]);
    expect(store.getState().selectedTransitionId).toBeNull();
    expect(screen.queryByRole("button", { name: /transition,/ })).not.toBeInTheDocument();
    expect(store.getState().history.past).toHaveLength(1);
    // The clips stay.
    expect(store.getState().project.timeline.tracks[0]?.items.map((item) => item.id)).toEqual(["a", "b"]);
  });

  it("restores a transition a move dropped with a single undo", async () => {
    const { store } = renderEditor();
    await act(async () => {
      await store.getState().applyActions([{ type: "moveItems", moves: [{ itemId: "b", targetTrackId: "v1", startSeconds: 3 }] }]);
    });
    expect(transitionsOf(store.getState().project)).toEqual([]);
    expect(store.getState().history.past).toHaveLength(1);
    await act(async () => {
      await store.getState().undo();
    });
    expect(transitionsOf(store.getState().project)).toEqual([crossfade()]);
    expect(screen.getByRole("button", { name: "Crossfade transition, 0.5s" })).toBeInTheDocument();
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("keeps clip clicks from selecting a transition", () => {
    const { store } = renderEditor(transitionTestProject([sourceClip("a", 0, 2, 0), sourceClip("b", 2, 2, 2)], [crossfade()]));
    act(() => store.getState().selectTransition("fade"));
    act(() => store.getState().selectItems(["a"]));
    expect(store.getState().selectedTransitionId).toBeNull();
    expect(screen.getByRole("button", { name: "Crossfade transition, 0.5s" })).toHaveAttribute("aria-pressed", "false");
  });
});
