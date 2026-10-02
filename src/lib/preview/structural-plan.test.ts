import { describe, expect, it, vi } from "vitest";
import { buildTimelinePreviewFrame } from "../timeline-preview";
import type { MediaAsset } from "../project";
import type { Timeline, TimelineItem } from "../timeline";
import * as transitions from "./transition-frame";

function fixture() {
  const media: MediaAsset[] = [{ id: "media", relativePath: "fixture.mp4", kind: "video", durationSeconds: 10, width: 1920, height: 1080, fps: 30, folderId: null }];
  const clip = (id: string, startSeconds: number): TimelineItem => ({ id, kind: "video_clip", label: id, startSeconds, durationSeconds: 4, source: { type: "media", mediaId: "media" }, properties: { sourceIn: 2, sourceOut: 6, keyframes: { positionX: [{ atSeconds: 0, value: 0 }, { atSeconds: 4, value: 40 }] } } });
  const timeline: Timeline = { durationSeconds: 8, tracks: [{ id: "video", name: "Video", kind: "video", locked: false, enabled: true, items: [clip("left", 0), clip("right", 4)], transitions: [{ id: "cut", leftItemId: "left", rightItemId: "right", kind: "crossfade", durationSeconds: 1 }] }] };
  return { timeline, media, fps: 30 };
}

describe("preview structural reuse", () => {
  it("indexes cut identities once instead of searching every item for every transition", () => {
    const input = fixture();
    const items = Array.from({ length: 20 }, (_, index) => ({ ...input.timeline.tracks[0]!.items[0]!, id: `clip-${index}`, startSeconds: index * 4 }));
    const track = { ...input.timeline.tracks[0]!, items, transitions: items.slice(1).map((item, index) => ({ id: `cut-${index}`, kind: "crossfade" as const, durationSeconds: 1, leftItemId: `clip-${index}`, rightItemId: item.id })) };
    let reads = 0;
    for (const item of items) { const id = item.id; Object.defineProperty(item, "id", { get: () => { reads += 1; return id; } }); }
    expect(transitions.planTrackTransitions(track, { media: input.media, generatedAssets: [] }, 1 / 30).size).toBe(20);
    expect(reads).toBeLessThan(240);
    reads = 0;
    expect(transitions.carryTransitionsThroughExpansion(track, items, "root", 1)).toHaveLength(19);
    expect(reads).toBeLessThan(100);
  });
  it("plans stable inputs once while reevaluating transitions, motion and source time at each playhead", () => {
    const input = fixture();
    const planning = vi.spyOn(transitions, "planTrackTransitions");
    try {
      const first = buildTimelinePreviewFrame({ ...input, playheadSeconds: 3.75 });
      const plannedCalls = planning.mock.calls.length;
      const second = buildTimelinePreviewFrame({ ...input, playheadSeconds: 4.25 });
      expect(second.transitions?.[0]?.progress).toBe(0.75);
      expect(first.transitions?.[0]?.progress).toBe(0.25);
      expect(second.layers.find((layer) => layer.itemId === "right")?.sourceTimeSeconds).toBe(2.25);
      expect(second.layers.find((layer) => layer.itemId === "right")?.positionX).toBe(2.5);
      expect(planning.mock.calls.length).toBe(plannedCalls);
    } finally { planning.mockRestore(); }
  });

  it("keeps only two strong plan entries so third-project visits evict the oldest plan", () => {
    const inputs = [fixture(), fixture(), fixture()];
    const planning = vi.spyOn(transitions, "planTrackTransitions");
    try {
      for (const input of inputs) buildTimelinePreviewFrame({ ...input, playheadSeconds: 4 });
      const plannedCalls = planning.mock.calls.length;
      buildTimelinePreviewFrame({ ...inputs[0]!, playheadSeconds: 4 });
      expect(planning.mock.calls.length).toBeGreaterThan(plannedCalls);
      const refreshedCalls = planning.mock.calls.length;
      buildTimelinePreviewFrame({ ...inputs[0]!, playheadSeconds: 4.2 });
      expect(planning.mock.calls.length).toBe(refreshedCalls);
    } finally { planning.mockRestore(); }
  });

  it("rebuilds after in-place item, transition, media and nested changes and new input identities", () => {
    const input = fixture();
    buildTimelinePreviewFrame({ ...input, playheadSeconds: 4 });
    const right = input.timeline.tracks[0]!.items[1]!;
    right.properties.positionX = 200;
    right.properties.keyframes = {};
    expect(buildTimelinePreviewFrame({ ...input, playheadSeconds: 4 }).layers.find((layer) => layer.itemId === "right")?.positionX).toBe(200);
    input.media[0]!.relativePath = "changed.mp4";
    expect(buildTimelinePreviewFrame({ ...input, playheadSeconds: 4 }).layers[0]?.relativePath).toBe("changed.mp4");
    input.timeline.tracks[0]!.transitions![0]!.kind = "wipe";
    expect(buildTimelinePreviewFrame({ ...input, playheadSeconds: 4 }).transitions?.[0]?.kind).toBe("wipe");
    expect(buildTimelinePreviewFrame({ ...input, media: [], playheadSeconds: 4 }).status).toBe("missing-media");
    const nested: Timeline = { durationSeconds: 8, tracks: [{ ...input.timeline.tracks[0]!, transitions: [], items: [{ ...right, id: "wrapper", source: { type: "timeline", timelineId: "child" }, startSeconds: 0, durationSeconds: 8, properties: {} }] }] };
    const timelines = [{ id: "child", timeline: input.timeline }];
    buildTimelinePreviewFrame({ ...input, timeline: nested, timelines, playheadSeconds: 4 });
    right.properties.positionX = 300;
    expect(buildTimelinePreviewFrame({ ...input, timeline: nested, timelines, playheadSeconds: 4 }).layers.find((layer) => layer.itemId.endsWith(":right"))?.positionX).toBe(300);
  });

  it("does not retain signatures above the bounded source budget", () => {
    const input = fixture();
    input.timeline.tracks[0]!.items[0]!.label = "x".repeat(2 * 1024 * 1024);
    const planning = vi.spyOn(transitions, "planTrackTransitions");
    try {
      buildTimelinePreviewFrame({ ...input, playheadSeconds: 4 });
      const plannedCalls = planning.mock.calls.length;
      buildTimelinePreviewFrame({ ...input, playheadSeconds: 4.2 });
      expect(planning.mock.calls.length).toBeGreaterThan(plannedCalls);
    } finally { planning.mockRestore(); }
  });

  it("does not retain expanded animation plans above the bounded allocation budget", () => {
    const input = fixture();
    const clip = input.timeline.tracks[0]!.items[0]!;
    const nested: Timeline = { durationSeconds: 840, tracks: [{ ...input.timeline.tracks[0]!, transitions: [], items: Array.from({ length: 210 }, (_, index) => ({ ...clip, id: `animated-${index}`, startSeconds: index * 4 })) }] };
    const timeline: Timeline = { durationSeconds: 840, tracks: [{ ...input.timeline.tracks[0]!, transitions: [], items: [{ ...clip, id: "wrapper", source: { type: "timeline", timelineId: "child" }, startSeconds: 0, durationSeconds: 840, properties: {} }] }] };
    const sources = { ...input, timeline, timelines: [{ id: "child", timeline: nested }] };
    const planning = vi.spyOn(transitions, "planTrackTransitions");
    try {
      expect(buildTimelinePreviewFrame({ ...sources, playheadSeconds: 4 }).status).toBe("ready");
      const plannedCalls = planning.mock.calls.length;
      expect(buildTimelinePreviewFrame({ ...sources, playheadSeconds: 4.2 }).status).toBe("ready");
      expect(planning.mock.calls.length).toBeGreaterThan(plannedCalls);
    } finally { planning.mockRestore(); }
  });

  it("invalidates signed-zero changes and keeps unrelated non-JSON properties usable", () => {
    const input = fixture();
    const clip = input.timeline.tracks[0]!.items[0]!;
    clip.properties.keyframes = {};
    clip.properties.positionX = -0;
    expect(Object.is(buildTimelinePreviewFrame({ ...input, playheadSeconds: 1 }).layers[0]?.positionX, -0)).toBe(true);
    clip.properties.positionX = 0;
    expect(Object.is(buildTimelinePreviewFrame({ ...input, playheadSeconds: 1 }).layers[0]?.positionX, 0)).toBe(true);
    clip.properties.unrelated = 1n;
    expect(buildTimelinePreviewFrame({ ...input, playheadSeconds: 1 }).status).toBe("ready");
  });
});
