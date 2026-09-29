import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { layoutModeForWidth, useLayoutMode } from "./use-layout-mode";

describe("layout mode", () => {
  afterEach(() => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 1024 });
  });

  it("maps widths to the two layouts and properties docking", () => {
    expect(layoutModeForWidth(402)).toBe("mobile");
    expect(layoutModeForWidth(1023)).toBe("mobile");
    expect(layoutModeForWidth(1024)).toBe("desktop-overlay");
    expect(layoutModeForWidth(1279)).toBe("desktop-overlay");
    expect(layoutModeForWidth(1280)).toBe("desktop-docked");
  });

  it("updates on resize", () => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 1440 });
    const { result } = renderHook(() => useLayoutMode());
    expect(result.current).toBe("desktop-docked");
    act(() => {
      Object.defineProperty(window, "innerWidth", { configurable: true, value: 402 });
      window.dispatchEvent(new Event("resize"));
    });
    expect(result.current).toBe("mobile");
  });
});
