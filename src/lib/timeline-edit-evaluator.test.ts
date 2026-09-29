import { describe, expect, it } from "vitest";
import type { Timeline } from "./timeline";
import {
  evaluateTimelineMove,
  evaluateTimelineResize,
} from "./timeline-edit-evaluator";

const timeline: Timeline = {
  durationSeconds: 8,
  tracks: [
    {
      id: "video",
      name: "Video",
      kind: "video",
      locked: false,
      items: [
        {
          id: "opening",
          kind: "video_clip",
          startSeconds: 0,
          durationSeconds: 2,
          source: { type: "media", mediaId: "opening" },
          label: "Opening clip",
          properties: {},
        },
        {
          id: "alternate",
          kind: "video_clip",
          startSeconds: 4,
          durationSeconds: 2,
          source: { type: "media", mediaId: "alternate" },
          label: "Restored Edison alternate",
          properties: {},
        },
      ],
    },
    {
      id: "video-locked",
      name: "Locked video",
      kind: "video",
      locked: true,
      items: [],
    },
    {
      id: "audio",
      name: "Audio",
      kind: "audio",
      locked: false,
      items: [],
    },
  ],
};

describe("evaluateTimelineMove", () => {
  it("accepts an exact seam and produces its commit patch", () => {
    expect(evaluateTimelineMove({
      timeline,
      itemId: "opening",
      targetTrackId: "video",
      proposedStartSeconds: 2,
      guideSeconds: 2,
    })).toMatchObject({
      state: "accepted",
      placements: [{
        itemId: "opening",
        trackId: "video",
        startSeconds: 2,
        durationSeconds: 2,
      }],
      patches: [{ itemId: "opening", targetTrackId: "video", startSeconds: 2 }],
      reason: null,
      guideSeconds: 2,
    });
  });

  it("rejects incompatible and locked target tracks", () => {
    expect(evaluateTimelineMove({
      timeline,
      itemId: "opening",
      targetTrackId: "audio",
      proposedStartSeconds: 2,
      guideSeconds: null,
    })).toMatchObject({
      state: "rejected",
      patches: [],
      reason: {
        code: "track_incompatible",
        trackId: "audio",
        blockingItemId: null,
        message: "Video clips stay on Video tracks",
      },
    });
    expect(evaluateTimelineMove({
      timeline,
      itemId: "opening",
      targetTrackId: "video-locked",
      proposedStartSeconds: 2,
      guideSeconds: null,
    })).toMatchObject({
      state: "rejected",
      patches: [],
      reason: {
        code: "track_locked",
        trackId: "video-locked",
        blockingItemId: null,
        message: "Track locked",
      },
    });
  });

  it("rejects a collision without producing a partial patch", () => {
    expect(evaluateTimelineMove({
      timeline,
      itemId: "opening",
      targetTrackId: "video",
      proposedStartSeconds: 3,
      guideSeconds: null,
    })).toMatchObject({
      state: "rejected",
      placements: [],
      patches: [],
      reason: {
        code: "collision",
        trackId: "video",
        blockingItemId: "alternate",
        message: "Overlaps Restored Edison alternate",
      },
    });
  });

  it("rejects a whole group when one placement enters an occupied destination", () => {
    const groupTimeline: Timeline = {
      ...timeline,
      tracks: timeline.tracks.map((track) => track.id === "video" ? {
        ...track,
        items: [
          ...track.items,
          {
            id: "second",
            kind: "video_clip" as const,
            startSeconds: 2,
            durationSeconds: 1,
            source: { type: "media" as const, mediaId: "second" },
            label: "Second clip",
            properties: {},
          },
        ],
      } : track),
    };

    expect(evaluateTimelineMove({
      timeline: groupTimeline,
      itemId: "opening",
      itemIds: ["opening", "second"],
      targetTrackId: "video",
      proposedStartSeconds: 3,
      guideSeconds: null,
    })).toMatchObject({ state: "rejected", placements: [], patches: [] });
  });

  it("preserves but does not increase a legacy overlap", () => {
    const legacyTimeline: Timeline = {
      ...timeline,
      tracks: timeline.tracks.map((track) => track.id === "video" ? {
        ...track,
        items: track.items.map((item) => item.id === "alternate" ? {
          ...item,
          startSeconds: 1.75,
        } : item),
      } : track),
    };

    expect(evaluateTimelineMove({
      timeline: legacyTimeline,
      itemId: "opening",
      targetTrackId: "video",
      proposedStartSeconds: 0,
      guideSeconds: null,
    })).toMatchObject({ state: "accepted", reason: null });
    expect(evaluateTimelineMove({
      timeline: legacyTimeline,
      itemId: "opening",
      targetTrackId: "video",
      proposedStartSeconds: 0.5,
      guideSeconds: null,
    })).toMatchObject({ state: "rejected", patches: [] });
  });
});

describe("evaluateTimelineResize", () => {
  it("does not create a persistence patch for a clamped no-op resize", () => {
    const result = evaluateTimelineResize({
      timeline,
      itemId: "opening",
      edge: "left",
      proposedStartSeconds: -1,
      proposedDurationSeconds: 3,
      guideSeconds: null,
    });

    expect(result.state).toBe("clamped");
    expect(result.placement).toMatchObject({ startSeconds: 0, durationSeconds: 2 });
    expect(result.patch).toBeNull();
  });
  it("clamps a right resize to the nearest stationary boundary", () => {
    const result = evaluateTimelineResize({
      timeline,
      itemId: "opening",
      edge: "right",
      proposedStartSeconds: 0,
      proposedDurationSeconds: 4.5,
      guideSeconds: null,
    });
    expect(result.state).toBe("clamped");
    expect(result.placement).toMatchObject({ startSeconds: 0, durationSeconds: 4 });
    expect(result.reason).toMatchObject({
      code: "collision",
      blockingItemId: "alternate",
      message: "Overlaps Restored Edison alternate",
    });
    expect(result.patch).toEqual({ type: "resizeItem", itemId: "opening", durationSeconds: 4 });
  });

  it("clamps a left resize past every earlier blocker to the nearest valid boundary", () => {
    const resizeTimeline: Timeline = {
      ...timeline,
      tracks: timeline.tracks.map((track) => track.id === "video" ? {
        ...track,
        items: [
          {
            id: "early",
            kind: "video_clip",
            startSeconds: 1,
            durationSeconds: 1,
            source: { type: "media", mediaId: "early" },
            label: "Early blocker",
            properties: {},
          },
          {
            id: "late",
            kind: "video_clip",
            startSeconds: 3,
            durationSeconds: 2,
            source: { type: "media", mediaId: "late" },
            label: "Late blocker",
            properties: {},
          },
          {
            id: "opening",
            kind: "video_clip",
            startSeconds: 6,
            durationSeconds: 2,
            source: { type: "media", mediaId: "opening" },
            label: "Opening clip",
            properties: { sourceIn: 6, sourceOut: 8 },
          },
        ],
      } : track),
    };

    expect(evaluateTimelineResize({
      timeline: resizeTimeline,
      itemId: "opening",
      edge: "left",
      proposedStartSeconds: 0,
      proposedDurationSeconds: 8,
      guideSeconds: null,
      sourceDurationSeconds: 10,
    })).toMatchObject({
      state: "clamped",
      placement: { startSeconds: 5, durationSeconds: 3 },
      patch: { type: "trimItem", startSeconds: 5, durationSeconds: 3, sourceIn: 5, sourceOut: 8 },
    });
  });

  it("does not shrink an unchanged legacy overlap while clamping a right resize", () => {
    const legacyTimeline: Timeline = {
      ...timeline,
      tracks: timeline.tracks.map((track) => track.id === "video" ? {
        ...track,
        items: track.items.map((item) => item.id === "alternate" ? {
          ...item,
          startSeconds: 1.75,
        } : item),
      } : track),
    };

    expect(evaluateTimelineResize({
      timeline: legacyTimeline,
      itemId: "opening",
      edge: "right",
      proposedStartSeconds: 0,
      proposedDurationSeconds: 3,
      guideSeconds: null,
    })).toMatchObject({
      state: "clamped",
      placement: { startSeconds: 0, durationSeconds: 2 },
      patch: null,
    });
  });

  it("creates speed-aware source range patches for left and right media trims", () => {
    const sourceTimeline: Timeline = {
      ...timeline,
      tracks: timeline.tracks.map((track) => track.id === "video" ? {
        ...track,
        items: track.items
          .filter((item) => item.id === "opening")
          .map((item) => ({
            ...item,
            startSeconds: 1,
            durationSeconds: 4,
            properties: { sourceIn: 2, sourceOut: 10, speed: 2 },
          })),
      } : track),
    };

    expect(evaluateTimelineResize({
      timeline: sourceTimeline,
      itemId: "opening",
      edge: "left",
      proposedStartSeconds: 2,
      proposedDurationSeconds: 3,
      guideSeconds: null,
      sourceDurationSeconds: 12,
    }).patch).toEqual({
      type: "trimItem",
      itemId: "opening",
      startSeconds: 2,
      durationSeconds: 3,
      sourceIn: 4,
      sourceOut: 10,
    });
    expect(evaluateTimelineResize({
      timeline: sourceTimeline,
      itemId: "opening",
      edge: "right",
      proposedStartSeconds: 1,
      proposedDurationSeconds: 5,
      guideSeconds: null,
      sourceDurationSeconds: 12,
    }).patch).toEqual({
      type: "trimItem",
      itemId: "opening",
      startSeconds: 1,
      durationSeconds: 5,
      sourceIn: 2,
      sourceOut: 12,
    });
  });

  it("clamps media trims to the 0.1-second minimum and source boundaries", () => {
    const sourceTimeline: Timeline = {
      ...timeline,
      tracks: timeline.tracks.map((track) => track.id === "video" ? {
        ...track,
        items: track.items
          .filter((item) => item.id === "opening")
          .map((item) => ({
            ...item,
            startSeconds: 1,
            durationSeconds: 4,
            properties: { sourceIn: 2, sourceOut: 10, speed: 2 },
          })),
      } : track),
    };

    expect(evaluateTimelineResize({
      timeline: sourceTimeline,
      itemId: "opening",
      edge: "right",
      proposedStartSeconds: 1,
      proposedDurationSeconds: 0,
      guideSeconds: null,
      sourceDurationSeconds: 12,
    })).toMatchObject({
      state: "clamped",
      placement: { startSeconds: 1, durationSeconds: 0.1 },
      patch: { sourceIn: 2, sourceOut: 2.2 },
    });
    expect(evaluateTimelineResize({
      timeline: sourceTimeline,
      itemId: "opening",
      edge: "left",
      proposedStartSeconds: -1,
      proposedDurationSeconds: 6,
      guideSeconds: null,
      sourceDurationSeconds: 12,
    })).toMatchObject({
      state: "clamped",
      placement: { startSeconds: 0, durationSeconds: 5 },
      patch: { sourceIn: 0, sourceOut: 10 },
    });
    expect(evaluateTimelineResize({
      timeline: sourceTimeline,
      itemId: "opening",
      edge: "right",
      proposedStartSeconds: 1,
      proposedDurationSeconds: 6,
      guideSeconds: null,
      sourceDurationSeconds: 12,
    })).toMatchObject({
      state: "clamped",
      placement: { startSeconds: 1, durationSeconds: 5 },
      patch: { sourceIn: 2, sourceOut: 12 },
    });
  });

  it("applies resolved snap deltas to move and resize geometry", () => {
    const snap = {
      probes: [{ seconds: 1.94, edge: "start" as const }],
      targets: [{ seconds: 2, kind: "seam" as const }],
      pixelsPerSecond: 100,
    };
    expect(evaluateTimelineMove({
      timeline,
      itemId: "opening",
      targetTrackId: "video",
      proposedStartSeconds: 1.94,
      guideSeconds: null,
      snap,
    })).toMatchObject({
      guideSeconds: 2,
      placements: [{ startSeconds: 2 }],
      patches: [{ startSeconds: 2 }],
    });
    expect(evaluateTimelineResize({
      timeline,
      itemId: "opening",
      edge: "right",
      proposedStartSeconds: 0,
      proposedDurationSeconds: 1.94,
      guideSeconds: null,
      snap: {
        ...snap,
        probes: [{ seconds: 1.94, edge: "end" as const }],
      },
    })).toMatchObject({
      guideSeconds: 2,
      placement: { startSeconds: 0, durationSeconds: 2 },
      patch: null,
    });
    expect(evaluateTimelineResize({
      timeline,
      itemId: "opening",
      edge: "left",
      proposedStartSeconds: 0.06,
      proposedDurationSeconds: 1.94,
      guideSeconds: null,
      snap: {
        ...snap,
        probes: [{ seconds: 0.06, edge: "start" as const }],
        targets: [{ seconds: 0, kind: "seam" as const }],
      },
    })).toMatchObject({
      state: "accepted",
      guideSeconds: 0,
      placement: { startSeconds: 0, durationSeconds: 2 },
      patch: null,
    });
  });
});
