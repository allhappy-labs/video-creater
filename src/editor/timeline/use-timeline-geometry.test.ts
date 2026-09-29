import { describe, expect, it } from "vitest";
import type { Timeline, TimelineItem, TimelineTrack } from "@/lib/timeline";
import {
  computeTimelineGeometry,
  fitZoom,
  keyframeLaneTrackIdFor,
  marqueeGeometry,
  timelineHeaderWidth,
  timelineMinimumRowHeight,
  timelineRowHeight,
  timelineRulerHeight,
  trackTargetAtY,
  visibleTimelineItems,
} from "./use-timeline-geometry";

function clip(id: string, startSeconds: number, durationSeconds: number): TimelineItem {
  return {
    id,
    kind: "video_clip",
    startSeconds,
    durationSeconds,
    source: { type: "media", mediaId: "media-1" },
    label: id,
    properties: {},
  };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[] = []): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items };
}

function timeline(tracks: TimelineTrack[], durationSeconds = 60): Timeline {
  return { durationSeconds, tracks };
}

describe("computeTimelineGeometry", () => {
  it("derives pixels per second from the zoom with an 80 px base", () => {
    const at100 = computeTimelineGeometry({ timeline: timeline([]), zoomPercent: 100, scrollLeft: 0, viewportWidth: 720, mobile: false });
    const at250 = computeTimelineGeometry({ timeline: timeline([]), zoomPercent: 250, scrollLeft: 0, viewportWidth: 720, mobile: false });
    expect(at100.pixelsPerSecond).toBe(80);
    expect(at250.pixelsPerSecond).toBe(200);
  });

  it("uses a 118 px header column on desktop and none on mobile", () => {
    expect(timelineHeaderWidth).toEqual({ desktop: 118, mobile: 0 });
    const desktop = computeTimelineGeometry({ timeline: timeline([]), zoomPercent: 100, scrollLeft: 0, viewportWidth: 720, mobile: false });
    const mobile = computeTimelineGeometry({ timeline: timeline([]), zoomPercent: 100, scrollLeft: 0, viewportWidth: 720, mobile: true });
    expect(desktop.headerWidth).toBe(118);
    expect(mobile.headerWidth).toBe(0);
  });

  it("pads mobile lanes by half the viewport on both sides so 0 and the end reach the centre", () => {
    const desktop = computeTimelineGeometry({ timeline: timeline([], 10), zoomPercent: 100, scrollLeft: 0, viewportWidth: 400, mobile: false });
    expect(desktop.centerPadding).toBe(0);
    expect(desktop.contentWidth).toBe(960);

    const mobile = computeTimelineGeometry({ timeline: timeline([], 10), zoomPercent: 100, scrollLeft: 5000, viewportWidth: 400, mobile: true });
    expect(mobile.centerPadding).toBe(200);
    // No trailing space: the right padding lets the end reach the centre.
    expect(mobile.contentWidth).toBe(800);
    // Scrolling ends with the last frame under the centre.
    expect(mobile.scrollLeft).toBe(800);
    expect(mobile.window.renderEndSeconds).toBe(10);
    expect(mobile.window.visibleEndSeconds).toBe(10);
  });

  it("stacks rows in band order with a tall main video row", () => {
    const geometry = computeTimelineGeometry({
      timeline: timeline([track("v1", "video"), track("a1", "audio"), track("t1", "overlay"), track("v2", "video")]),
      zoomPercent: 100,
      scrollLeft: 0,
      viewportWidth: 720,
      mobile: false,
    });
    expect(timelineRowHeight).toEqual({ mainVideo: 58, other: 34, keyframeLane: 40 });
    expect(geometry.rows.map((row) => [row.track.id, row.name, row.top, row.height, row.main])).toEqual([
      ["t1", "Text 1", 0, 34, false],
      ["v1", "Video 1", 34, 58, true],
      ["v2", "Video 2", 92, 34, false],
      ["a1", "Audio 1", 126, 34, false],
    ]);
    expect(geometry.totalHeight).toBe(160);
  });

  describe("fitting rows to the viewport height", () => {
    // Flow 1 at 1440×900: the default 300 px split leaves 226 px under the toolbar and above the
    // overview bar, and an inserted clip's new track makes six rows (252 px with the ruler).
    const sixRows = timeline([
      track("g1", "hyperframe_scene"),
      track("t1", "overlay"),
      track("c1", "caption"),
      track("v1", "video"),
      track("v2", "video"),
      track("a1", "audio"),
    ]);
    const geometryAt = (viewportHeight: number, mobile = false, keyframeLaneTrackId: string | null = null) =>
      computeTimelineGeometry({ timeline: sixRows, zoomPercent: 100, scrollLeft: 0, viewportWidth: 720, viewportHeight, mobile, keyframeLaneTrackId });

    it("keeps nominal heights when the rows fit or the height is unmeasured", () => {
      for (const height of [0, 252, 600]) {
        const geometry = geometryAt(height);
        expect(geometry.rows.map((row) => row.height)).toEqual([34, 34, 34, 58, 34, 34]);
        expect(geometry.totalHeight).toBe(228);
      }
    });

    it("compacts rows so every row fits above the overview bar", () => {
      const geometry = geometryAt(226);
      expect(timelineRulerHeight + geometry.totalHeight).toBeLessThanOrEqual(226);
      const last = geometry.rows[geometry.rows.length - 1];
      expect(last && timelineRulerHeight + last.top + last.height).toBeLessThanOrEqual(226);
      for (const row of geometry.rows) {
        expect(row.height).toBeGreaterThanOrEqual(row.main ? timelineMinimumRowHeight.desktop.mainVideo : timelineMinimumRowHeight.desktop.other);
      }
      // Rows stay contiguous so hit-testing and marquee boxes match what is drawn.
      geometry.rows.forEach((row, index) => expect(row.top).toBe(index === 0 ? 0 : (geometry.rows[index - 1]?.top ?? 0) + (geometry.rows[index - 1]?.height ?? 0)));
    });

    it("stops at the minimum heights and leaves the rest to vertical scrolling", () => {
      const geometry = geometryAt(120);
      expect(geometry.rows.map((row) => row.height)).toEqual([28, 28, 28, 46, 28, 28]);
      expect(timelineRulerHeight + geometry.totalHeight).toBeGreaterThan(120);
    });

    it("keeps touch rows tall enough for 24 px transition handles and leaves the keyframe lane alone", () => {
      const touch = geometryAt(120, true);
      expect(touch.rows.map((row) => row.height)).toEqual([30, 30, 30, 48, 30, 30]);
      const withLane = geometryAt(266, false, "v1");
      expect(withLane.keyframeLane?.height).toBe(timelineRowHeight.keyframeLane);
      expect(timelineRulerHeight + withLane.totalHeight).toBeLessThanOrEqual(266);
    });
  });

  it("windows the viewport with one viewport of overscan", () => {
    const geometry = computeTimelineGeometry({
      timeline: timeline([]),
      zoomPercent: 100,
      scrollLeft: 800,
      viewportWidth: 720,
      mobile: false,
    });
    expect(geometry.window).toEqual({
      visibleStartSeconds: 10,
      visibleEndSeconds: 19,
      renderStartSeconds: 1,
      renderEndSeconds: 28,
    });
  });

  it("clamps the effective scroll to the scrollable content", () => {
    const geometry = computeTimelineGeometry({
      timeline: timeline([], 10),
      zoomPercent: 100,
      scrollLeft: 99_999,
      viewportWidth: 720,
      mobile: false,
    });
    expect(geometry.contentWidth).toBe(10 * 80 + 160);
    expect(geometry.scrollLeft).toBe(geometry.contentWidth - 720);
  });

  it("never makes the content narrower than the viewport", () => {
    const geometry = computeTimelineGeometry({ timeline: timeline([], 2), zoomPercent: 100, scrollLeft: 0, viewportWidth: 900, mobile: false });
    expect(geometry.contentWidth).toBe(900);
    expect(geometry.renderedDurationSeconds).toBe(900 / 80);
  });

  it("converts between seconds and content pixels", () => {
    const geometry = computeTimelineGeometry({ timeline: timeline([]), zoomPercent: 50, scrollLeft: 0, viewportWidth: 720, mobile: false });
    expect(geometry.secondsToX(3)).toBe(120);
    expect(geometry.xToSeconds(120)).toBe(3);
  });
});

describe("visibleTimelineItems", () => {
  const window = { visibleStartSeconds: 10, visibleEndSeconds: 19, renderStartSeconds: 1, renderEndSeconds: 28 };

  it("keeps items inside the render window plus persistent ids", () => {
    const items = [clip("before", 0, 0.5), clip("inside", 5, 2), clip("after", 40, 2), clip("selected", 50, 2)];
    const visible = visibleTimelineItems(items, window, new Set(["selected"]));
    expect(visible.map((item) => item.id)).toEqual(["inside", "selected"]);
  });
});

describe("fitZoom", () => {
  it("fits the duration and the trailing space into the viewport", () => {
    expect(fitZoom(10, 960)).toBe(100);
    expect(fitZoom(100, 960)).toBe(10);
  });

  it("clamps to the zoom bounds and falls back for empty timelines", () => {
    expect(fitZoom(0.1, 1920)).toBe(1000);
    expect(fitZoom(10_000, 960)).toBe(10);
    expect(fitZoom(0, 960)).toBe(100);
  });
});

describe("trackTargetAtY", () => {
  // Rows: t1 0–34, v1 34–92 (main), a1 92–126.
  const geometry = computeTimelineGeometry({
    timeline: timeline([track("v1", "video", [clip("a", 1, 2)]), track("t1", "overlay"), track("a1", "audio")]),
    zoomPercent: 100,
    scrollLeft: 0,
    viewportWidth: 720,
    mobile: false,
  });

  it("targets the row under the point, or an insert index near row edges and outside the rows", () => {
    expect(trackTargetAtY(geometry.rows, 60)).toEqual({ trackId: "v1", insertIndex: null });
    expect(trackTargetAtY(geometry.rows, 36)).toEqual({ trackId: "v1", insertIndex: 1 });
    expect(trackTargetAtY(geometry.rows, 90)).toEqual({ trackId: "v1", insertIndex: 2 });
    expect(trackTargetAtY(geometry.rows, -4)).toEqual({ trackId: "t1", insertIndex: 0 });
    expect(trackTargetAtY(geometry.rows, 400)).toEqual({ trackId: "a1", insertIndex: 3 });
    expect(trackTargetAtY([], 10)).toBeNull();
  });

  it("treats the keyframe lane below a row as the boundary after that row", () => {
    const withLane = computeTimelineGeometry({
      timeline: timeline([track("v1", "video", [clip("a", 1, 2)]), track("t1", "overlay"), track("a1", "audio")]),
      zoomPercent: 100,
      scrollLeft: 0,
      viewportWidth: 720,
      mobile: false,
      keyframeLaneTrackId: "v1",
    });
    expect(withLane.keyframeLane).toEqual({ trackId: "v1", top: 92, height: 40 });
    expect(withLane.rows.map((row) => row.top)).toEqual([0, 34, 132]);
    expect(withLane.totalHeight).toBe(166);
    expect(trackTargetAtY(withLane.rows, 110)).toEqual({ trackId: "v1", insertIndex: 2 });
    expect(trackTargetAtY(withLane.rows, 150)).toEqual({ trackId: "a1", insertIndex: null });
  });

  it("builds marquee boxes from rows and clips", () => {
    expect(marqueeGeometry(geometry)).toEqual({
      rows: [
        { trackId: "t1", top: 0, bottom: 34 },
        { trackId: "v1", top: 34, bottom: 92 },
        { trackId: "a1", top: 92, bottom: 126 },
      ],
      items: [{ itemId: "a", trackId: "v1", left: 80, right: 240 }],
      verticalInset: 3,
    });
  });
});

describe("keyframeLaneTrackIdFor", () => {
  const tracks = timeline([track("v1", "video", [clip("a", 0, 1), clip("b", 2, 1)])]);

  it("is the selected clip's track only when the lane is on and exactly one clip is selected", () => {
    expect(keyframeLaneTrackIdFor(tracks, true, ["a"])).toBe("v1");
    expect(keyframeLaneTrackIdFor(tracks, false, ["a"])).toBeNull();
    expect(keyframeLaneTrackIdFor(tracks, true, [])).toBeNull();
    expect(keyframeLaneTrackIdFor(tracks, true, ["a", "b"])).toBeNull();
    expect(keyframeLaneTrackIdFor(tracks, true, ["missing"])).toBeNull();
  });
});
