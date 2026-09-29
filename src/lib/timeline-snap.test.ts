import { describe, expect, it } from "vitest";
import { resolveTimelineSnap, timelineSnapTargets } from "./timeline-snap";

describe("resolveTimelineSnap", () => {
  it("prioritizes the closest clip edge", () => {
    expect(resolveTimelineSnap({
      probes: [{ seconds: 1.94, edge: "start" }],
      targets: [{ seconds: 2, kind: "edit" }],
      pixelsPerSecond: 100,
    })).toMatchObject({ deltaSeconds: 0.06, guideSeconds: 2, targetKind: "edit" });
  });

  it("uses playhead priority when candidates are equally close", () => {
    expect(resolveTimelineSnap({
      probes: [{ seconds: 2, edge: "start" }],
      targets: [
        { seconds: 1.95, kind: "edit" },
        { seconds: 2.05, kind: "playhead" },
      ],
      pixelsPerSecond: 100,
    })).toMatchObject({ deltaSeconds: 0.05, guideSeconds: 2.05, targetKind: "playhead" });
  });

  it("uses a neighboring seam before a generic edit point at equal distance", () => {
    expect(resolveTimelineSnap({
      probes: [{ seconds: 1, edge: "end" }],
      targets: [
        { seconds: 0.5, kind: "edit" },
        { seconds: 1.5, kind: "seam" },
      ],
      pixelsPerSecond: 10,
    })).toMatchObject({ deltaSeconds: 0.5, guideSeconds: 1.5, targetKind: "seam" });
  });

  it("derives the threshold from zoom", () => {
    const options = {
      probes: [{ seconds: 1.9, edge: "start" as const }],
      targets: [{ seconds: 2, kind: "edit" as const }],
    };
    expect(resolveTimelineSnap({ ...options, pixelsPerSecond: 100 }).guideSeconds).toBeNull();
    expect(resolveTimelineSnap({ ...options, pixelsPerSecond: 60 }).guideSeconds).toBe(2);
  });

  it("retains a sticky target until the wider release threshold is crossed", () => {
    const targets = [{ seconds: 2, kind: "edit" as const }];
    const acquired = resolveTimelineSnap({
      probes: [{ seconds: 1.94, edge: "end" }],
      targets,
      pixelsPerSecond: 100,
    });
    expect(resolveTimelineSnap({
      probes: [{ seconds: 1.88, edge: "end" }],
      targets,
      pixelsPerSecond: 100,
      sticky: acquired.sticky,
    }).guideSeconds).toBe(2);
    expect(resolveTimelineSnap({
      probes: [{ seconds: 1.8, edge: "end" }],
      targets,
      pixelsPerSecond: 100,
      sticky: acquired.sticky,
    }).guideSeconds).toBeNull();
  });

  it("builds unique targets with the playhead first", () => {
    expect(timelineSnapTargets([0, 1, 2, 2], 1)).toEqual([
      { seconds: 1, kind: "playhead" },
      { seconds: 0, kind: "edit" },
      { seconds: 2, kind: "edit" },
    ]);
  });
});
