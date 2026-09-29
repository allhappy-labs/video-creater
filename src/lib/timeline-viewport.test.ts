import { describe, expect, it } from "vitest";
import {
  adaptiveTimelineTicks,
  timelineItemIntersectsWindow,
  timelineViewportWindow,
  visibleTimelineTrackRange,
} from "./timeline-viewport";

describe("timeline viewport", () => {
  it("bounds a 30-minute project to the viewport plus one viewport of overscan", () => {
    expect(timelineViewportWindow({
      durationSeconds: 1_800,
      pixelsPerSecond: 80,
      scrollLeft: 8_000,
      viewportWidth: 1_200,
      overscanViewports: 1,
    })).toEqual({
      visibleStartSeconds: 100,
      visibleEndSeconds: 115,
      renderStartSeconds: 85,
      renderEndSeconds: 130,
    });
  });

  it("keeps combined visible ruler and grid ticks below 400 at normal zoom", () => {
    const window = timelineViewportWindow({
      durationSeconds: 1_800,
      pixelsPerSecond: 80,
      scrollLeft: 0,
      viewportWidth: 1_200,
      overscanViewports: 1,
    });
    const ticks = adaptiveTimelineTicks({ window, pixelsPerSecond: 80, snapSeconds: 0.25 });
    expect(ticks.majorSeconds.length + ticks.minorSeconds.length).toBeLessThanOrEqual(200);
  });

  it("caps combined ruler and grid ticks across an overscanned low-zoom viewport", () => {
    const window = timelineViewportWindow({
      durationSeconds: 1_800,
      pixelsPerSecond: 8,
      scrollLeft: 7_200,
      viewportWidth: 1_440,
      overscanViewports: 1,
    });
    const ticks = adaptiveTimelineTicks({ window, pixelsPerSecond: 8, snapSeconds: 1 });

    expect(ticks.majorSeconds.length + ticks.minorSeconds.length).toBeLessThanOrEqual(200);
  });

  it("selects a candidate-aligned major interval that fits a high-overscan tick budget", () => {
    const window = timelineViewportWindow({
      durationSeconds: 1_800,
      pixelsPerSecond: 80,
      scrollLeft: 72_000,
      viewportWidth: 1_440,
      overscanViewports: 10,
    });
    const ticks = adaptiveTimelineTicks({ window, pixelsPerSecond: 80, snapSeconds: 0.25 });

    expect(ticks.majorSeconds).toEqual(Array.from({ length: 190 }, (_, index) => 720 + index * 2));
  });

  it("clamps overscroll to the project end while keeping viewport ranges ordered", () => {
    expect(timelineViewportWindow({
      durationSeconds: 1_800,
      pixelsPerSecond: 8,
      scrollLeft: 99_999,
      viewportWidth: 1_440,
      overscanViewports: 1,
    })).toEqual({
      visibleStartSeconds: 1_620,
      visibleEndSeconds: 1_800,
      renderStartSeconds: 1_440,
      renderEndSeconds: 1_800,
    });
  });

  it("normalizes a negative duration to an empty ordered viewport", () => {
    expect(timelineViewportWindow({
      durationSeconds: -30,
      pixelsPerSecond: 8,
      scrollLeft: 500,
      viewportWidth: 1_440,
      overscanViewports: 1,
    })).toEqual({
      visibleStartSeconds: 0,
      visibleEndSeconds: 0,
      renderStartSeconds: 0,
      renderEndSeconds: 0,
    });
  });

  it("keeps selected items mounted outside the overscan window", () => {
    const item = { id: "selected", startSeconds: 500, durationSeconds: 4 };
    expect(timelineItemIntersectsWindow(item, { renderStartSeconds: 0, renderEndSeconds: 30 }, new Set(["selected"]))).toBe(true);
  });

  it("describes all tracks when the vertical viewport contains the full stack", () => {
    expect(visibleTimelineTrackRange({
      entries: [
        { top: 0, bottom: 64 },
        { top: 64, bottom: 112 },
        { top: 112, bottom: 160 },
      ],
      scrollTop: 0,
      viewportHeight: 240,
    })).toEqual({
      firstTrackNumber: 1,
      lastTrackNumber: 3,
      totalTracks: 3,
      label: "3 tracks",
    });
  });

  it("describes the intersecting middle tracks after vertical scrolling", () => {
    expect(visibleTimelineTrackRange({
      entries: [
        { top: 0, bottom: 64 },
        { top: 64, bottom: 112 },
        { top: 112, bottom: 160 },
        { top: 160, bottom: 208 },
        { top: 208, bottom: 272 },
      ],
      scrollTop: 96,
      viewportHeight: 96,
    })).toEqual({
      firstTrackNumber: 2,
      lastTrackNumber: 4,
      totalTracks: 5,
      label: "Tracks 2–4 of 5",
    });
  });

  it("returns an explicit empty track range", () => {
    expect(visibleTimelineTrackRange({
      entries: [],
      scrollTop: 0,
      viewportHeight: 100,
    })).toEqual({
      firstTrackNumber: 0,
      lastTrackNumber: 0,
      totalTracks: 0,
      label: "No tracks",
    });
  });
});
