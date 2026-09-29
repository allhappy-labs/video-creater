import { describe, expect, it } from "vitest";
import {
  isMarqueeClick,
  itemsInMarquee,
  type MarqueeGeometry,
} from "@/lib/timeline-ops/marquee";

// Row "a" spans y 0..40 (item band 4..36); row "b" spans y 40..100 (item band 44..96).
const geometry: MarqueeGeometry = {
  rows: [
    { trackId: "a", top: 0, bottom: 40 },
    { trackId: "b", top: 40, bottom: 100 },
    // Scrolled far below the visible canvas.
    { trackId: "far", top: 2000, bottom: 2040 },
  ],
  items: [
    { itemId: "a1", trackId: "a", left: 0, right: 100 },
    { itemId: "a2", trackId: "a", left: 150, right: 250 },
    { itemId: "b1", trackId: "b", left: 50, right: 120 },
    { itemId: "far1", trackId: "far", left: 0, right: 500 },
    { itemId: "orphan", trackId: "missing", left: 0, right: 500 },
  ],
};

describe("itemsInMarquee", () => {
  it("selects items that partially overlap the rect in both axes", () => {
    expect(itemsInMarquee(geometry, { startX: 90, startY: 30, endX: 160, endY: 50 })).toEqual([
      "a1",
      "a2",
      "b1",
    ]);
  });

  it("normalizes rects dragged from bottom-right to top-left", () => {
    expect(itemsInMarquee(geometry, { startX: 160, startY: 20, endX: 90, endY: 10 })).toEqual([
      "a1",
      "a2",
    ]);
  });

  it("treats edges as inclusive and applies the vertical row inset", () => {
    // Touches a1's right edge exactly, and a1's inset top edge at y 36.
    expect(itemsInMarquee(geometry, { startX: 100, startY: 36, endX: 140, endY: 38 })).toEqual(["a1"]);
    // Inside row "a" but within the bottom inset (36..40): nothing.
    expect(itemsInMarquee(geometry, { startX: 0, startY: 37, endX: 300, endY: 39 })).toEqual([]);
  });

  it("handles zero-size rects inclusively", () => {
    expect(itemsInMarquee(geometry, { startX: 100, startY: 20, endX: 100, endY: 20 })).toEqual(["a1"]);
    expect(itemsInMarquee(geometry, { startX: 125, startY: 20, endX: 125, endY: 20 })).toEqual([]);
  });

  it("ignores items on off-screen rows unless the rect reaches them, and items without a row", () => {
    expect(itemsInMarquee(geometry, { startX: 0, startY: 0, endX: 600, endY: 200 })).toEqual([
      "a1",
      "a2",
      "b1",
    ]);
    expect(itemsInMarquee(geometry, { startX: 0, startY: 1990, endX: 10, endY: 2010 })).toEqual(["far1"]);
  });

  it("keeps at least 1px of item height for rows thinner than the insets", () => {
    const thin: MarqueeGeometry = {
      rows: [{ trackId: "t", top: 10, bottom: 14 }],
      items: [{ itemId: "t1", trackId: "t", left: 0, right: 10 }],
    };
    expect(itemsInMarquee(thin, { startX: 5, startY: 15, endX: 5, endY: 15 })).toEqual(["t1"]);
    expect(itemsInMarquee(thin, { startX: 5, startY: 16, endX: 5, endY: 16 })).toEqual([]);
  });
});

describe("isMarqueeClick", () => {
  it("counts movement under 3px as a click", () => {
    expect(isMarqueeClick({ startX: 10, startY: 10, endX: 12, endY: 12 })).toBe(true);
    expect(isMarqueeClick({ startX: 10, startY: 10, endX: 13, endY: 10 })).toBe(false);
  });
});
