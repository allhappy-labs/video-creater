import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { installPointerEventPolyfill } from "@/test-utils/editor-render";
import { BottomSheet, sheetDragOffset, sheetHeightClass, sheetSwipeClosePixels } from "./bottom-sheet";

function renderSheet(props: { readonly height?: "compact" | "tall"; readonly aboveToolBar?: boolean } = {}) {
  const onClose = vi.fn();
  render(
    <BottomSheet title="Media" open height={props.height ?? "tall"} aboveToolBar={props.aboveToolBar ?? false} onClose={onClose}>
      <p>Body</p>
    </BottomSheet>,
  );
  return { onClose, sheet: screen.getByRole("dialog", { name: "Media" }), handle: screen.getByRole("button", { name: "Dismiss Media sheet" }) };
}

function drag(handle: HTMLElement, fromY: number, toY: number, end: "up" | "cancel" = "up") {
  fireEvent.pointerDown(handle, { pointerId: 4, pointerType: "touch", clientX: 200, clientY: fromY });
  fireEvent.pointerMove(handle, { pointerId: 4, pointerType: "touch", clientX: 200, clientY: toY });
  if (end === "up") fireEvent.pointerUp(handle, { pointerId: 4, pointerType: "touch", clientX: 200, clientY: toY });
  else fireEvent.pointerCancel(handle, { pointerId: 4, pointerType: "touch" });
}

describe("BottomSheet", () => {
  beforeAll(() => installPointerEventPolyfill());

  it("follows a downward handle drag and closes past 80 px", () => {
    const { onClose, sheet, handle } = renderSheet();
    fireEvent.pointerDown(handle, { pointerId: 4, pointerType: "touch", clientX: 200, clientY: 500 });
    fireEvent.pointerMove(handle, { pointerId: 4, pointerType: "touch", clientX: 200, clientY: 560 });
    expect(sheet).toHaveStyle({ transform: "translateY(60px)" });
    fireEvent.pointerMove(handle, { pointerId: 4, pointerType: "touch", clientX: 200, clientY: 500 + sheetSwipeClosePixels + 1 });
    fireEvent.pointerUp(handle, { pointerId: 4, pointerType: "touch", clientX: 200, clientY: 500 + sheetSwipeClosePixels + 1 });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("springs back from a short or cancelled drag without closing, even on the click that ends it", () => {
    const { onClose, sheet, handle } = renderSheet();
    drag(handle, 500, 500 + sheetSwipeClosePixels);
    fireEvent.click(handle);
    expect(sheet.style.transform).toBe("");
    drag(handle, 500, 700, "cancel");
    expect(sheet.style.transform).toBe("");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes when the handle is tapped or activated from the keyboard", () => {
    const { onClose, handle } = renderSheet();
    fireEvent.click(handle);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("only follows downward travel", () => {
    expect(sheetDragOffset(500, 420)).toBe(0);
    expect(sheetDragOffset(500, 590)).toBe(90);
  });

  it("uses 40% and 70% heights, 55% on viewports taller than 1000 px, and clears the home indicator", () => {
    expect(sheetHeightClass.compact).toContain("h-[40dvh]");
    expect(sheetHeightClass.tall).toContain("h-[70dvh]");
    for (const classes of Object.values(sheetHeightClass)) expect(classes).toContain("[@media(min-height:1001px)]:h-[55dvh]");

    const { sheet } = renderSheet({ height: "tall" });
    expect(sheet).toHaveClass("h-[70dvh]", "[@media(min-height:1001px)]:h-[55dvh]", "pb-[env(safe-area-inset-bottom)]");
  });

  it("sits above the tool bar and its safe-area inset when asked", () => {
    const { sheet } = renderSheet({ height: "compact", aboveToolBar: true });
    expect(sheet).toHaveClass("h-[40dvh]");
    expect(sheet).not.toHaveClass("pb-[env(safe-area-inset-bottom)]");
    expect(sheet.style.bottom).toBe("calc(78px + env(safe-area-inset-bottom))");
  });
});
