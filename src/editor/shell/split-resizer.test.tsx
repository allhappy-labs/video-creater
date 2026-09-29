import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { SplitResizer } from "./split-resizer";

describe("SplitResizer", () => {
  beforeAll(() => {
    // jsdom has no PointerEvent, so fireEvent.pointer* would drop clientX/clientY and pointerId.
    if (typeof window.PointerEvent === "undefined") {
      class PointerEventPolyfill extends MouseEvent {
        readonly pointerId: number;
        constructor(type: string, init: PointerEventInit = {}) {
          super(type, init);
          this.pointerId = init.pointerId ?? 0;
        }
      }
      window.PointerEvent = PointerEventPolyfill as unknown as typeof PointerEvent;
    }
  });

  it("reports pointer drag deltas along its axis and supports arrow keys", () => {
    const onResize = vi.fn();
    render(<SplitResizer orientation="vertical" label="Resize left panel" value={360} min={300} max={440} onResize={onResize} />);
    const handle = screen.getByRole("separator", { name: "Resize left panel" });
    expect(handle).toHaveAttribute("aria-valuenow", "360");

    fireEvent.pointerDown(handle, { clientX: 100, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 130, clientY: 0, pointerId: 1 });
    expect(onResize).toHaveBeenLastCalledWith(390);
    fireEvent.pointerUp(handle, { pointerId: 1 });

    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(onResize).toHaveBeenLastCalledWith(350);
  });

  it("inverts deltas for handles that grow toward the top", () => {
    const onResize = vi.fn();
    render(<SplitResizer orientation="horizontal" invert label="Resize timeline" value={300} min={160} max={600} onResize={onResize} />);
    const handle = screen.getByRole("separator", { name: "Resize timeline" });
    fireEvent.pointerDown(handle, { clientX: 0, clientY: 500, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 0, clientY: 450, pointerId: 1 });
    expect(onResize).toHaveBeenLastCalledWith(350);
  });
});
