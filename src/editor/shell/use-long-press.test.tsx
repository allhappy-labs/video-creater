import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { installPointerEventPolyfill } from "@/test-utils/editor-render";
import { exceedsLongPressTolerance, longPressMilliseconds, longPressTolerancePixels, useLongPress, type LongPressPoint } from "./use-long-press";

function Pressable({ onLongPress, onClick }: { readonly onLongPress: (press: LongPressPoint) => void; readonly onClick: () => void }) {
  const { handlers } = useLongPress({ onLongPress });
  return (
    <button type="button" onClick={onClick} {...handlers}>
      Target
    </button>
  );
}

function renderPressable() {
  const onLongPress = vi.fn<(press: LongPressPoint) => void>();
  const onClick = vi.fn();
  render(<Pressable onLongPress={onLongPress} onClick={onClick} />);
  return { onLongPress, onClick, target: screen.getByRole("button", { name: "Target" }) };
}

function press(target: Element, pointerType = "touch", clientX = 100, clientY = 50) {
  fireEvent.pointerDown(target, { pointerId: 7, pointerType, clientX, clientY });
}

describe("useLongPress", () => {
  beforeAll(() => installPointerEventPolyfill());
  beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
  afterEach(() => vi.useRealTimers());

  it("fires after a 500 ms touch press with the press point", () => {
    const { onLongPress, target } = renderPressable();
    press(target);
    act(() => vi.advanceTimersByTime(longPressMilliseconds - 1));
    expect(onLongPress).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onLongPress).toHaveBeenCalledTimes(1);
    expect(onLongPress).toHaveBeenCalledWith({ target, pointerId: 7, clientX: 100, clientY: 50 });
  });

  it("tolerates 8 px of finger travel and cancels beyond it", () => {
    const { onLongPress, target } = renderPressable();
    press(target);
    fireEvent.pointerMove(target, { pointerId: 7, pointerType: "touch", clientX: 100 + longPressTolerancePixels, clientY: 50 });
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(onLongPress).toHaveBeenCalledTimes(1);

    press(target);
    fireEvent.pointerMove(target, { pointerId: 7, pointerType: "touch", clientX: 100, clientY: 50 + longPressTolerancePixels + 1 });
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(onLongPress).toHaveBeenCalledTimes(1);
  });

  it("cancels when the finger lifts or the pointer is cancelled", () => {
    const { onLongPress, target } = renderPressable();
    press(target);
    fireEvent.pointerUp(target, { pointerId: 7, pointerType: "touch" });
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    press(target);
    fireEvent.pointerCancel(target, { pointerId: 7, pointerType: "touch" });
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(onLongPress).not.toHaveBeenCalled();
  });

  it("ignores mouse presses and lets pen presses through", () => {
    const { onLongPress, target } = renderPressable();
    press(target, "mouse");
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(onLongPress).not.toHaveBeenCalled();
    press(target, "pen");
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    expect(onLongPress).toHaveBeenCalledTimes(1);
  });

  it("swallows the click that ends a long press but not later taps", () => {
    const { onClick, target } = renderPressable();
    press(target);
    act(() => vi.advanceTimersByTime(longPressMilliseconds));
    fireEvent.click(target);
    expect(onClick).not.toHaveBeenCalled();
    fireEvent.click(target);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("measures travel as a straight-line distance", () => {
    expect(exceedsLongPressTolerance({ clientX: 0, clientY: 0 }, { clientX: 6, clientY: 5 })).toBe(false);
    expect(exceedsLongPressTolerance({ clientX: 0, clientY: 0 }, { clientX: 6, clientY: 6 })).toBe(true);
  });
});
