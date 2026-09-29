import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import { evaluateClipRippleTrim, evaluateClipTrim } from "@/lib/timeline-ops/clip-trim";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function item(id: string, kind: TimelineItemKind, startSeconds: number, durationSeconds: number, properties: Record<string, unknown> = {}): TimelineItem {
  return { id, kind, startSeconds, durationSeconds, source: { type: "text", text: id }, label: id, properties };
}

function mediaItem(id: string, startSeconds: number, durationSeconds: number, properties: Record<string, unknown> = {}): TimelineItem {
  // fixtureProject() media "media-1" is a 4 second video.
  return { ...item(id, "video_clip", startSeconds, durationSeconds, properties), source: { type: "media", mediaId: "media-1" } };
}

function track(id: string, kind: TrackKind, items: TimelineItem[], patch: Partial<TimelineTrack> = {}): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items, ...patch };
}

function projectWith(tracks: TimelineTrack[]): VideoProject {
  const project = fixtureProject();
  project.timeline = { durationSeconds: 60, tracks };
  return project;
}

describe("evaluateClipTrim", () => {
  it("trims the left edge into a trimItems action that moves sourceIn by the speed", () => {
    const project = projectWith([track("v1", "video", [mediaItem("a", 1, 3, { sourceIn: 0.5, sourceOut: 3.5 })])]);
    const preview = evaluateClipTrim({ project, itemId: "a", edge: "left", deltaSeconds: 0.5, snap: null });
    expect(preview.state).toBe("accepted");
    expect(preview.placement).toMatchObject({ startSeconds: 1.5, durationSeconds: 2.5 });
    expect(preview.action).toEqual({
      type: "trimItems",
      trims: [{ itemId: "a", startSeconds: 1.5, durationSeconds: 2.5, sourceIn: 1, sourceOut: 3.5 }],
    });
  });

  it("resizes the right edge of a clip without a source range, keeping at least 0.1 s", () => {
    const project = projectWith([track("t1", "overlay", [item("title", "overlay", 2, 2)])]);
    const preview = evaluateClipTrim({ project, itemId: "title", edge: "right", deltaSeconds: -5, snap: null });
    expect(preview.placement.durationSeconds).toBe(0.1);
    expect(preview.action).toEqual({ type: "resizeItems", resizes: [{ itemId: "title", durationSeconds: 0.1 }] });
  });

  it("clamps the right edge to the source media and to the next clip, with a reason", () => {
    const project = projectWith([track("v1", "video", [mediaItem("a", 0, 2, { sourceIn: 0, sourceOut: 2 }), item("b", "video_clip", 3, 2)])]);
    const collision = evaluateClipTrim({ project, itemId: "a", edge: "right", deltaSeconds: 1.5, snap: null });
    expect(collision.state).toBe("clamped");
    expect(collision.placement.durationSeconds).toBe(3);
    expect(collision.reason).toBe("Overlaps b");

    project.timeline.tracks[0]?.items.pop();
    const source = evaluateClipTrim({ project, itemId: "a", edge: "right", deltaSeconds: 5, snap: null });
    expect(source.state).toBe("clamped");
    expect(source.placement.durationSeconds).toBe(4);
    expect(source.reason).toBe("Resize limited to the available range");
  });

  it("snaps the dragged edge to the playhead", () => {
    const project = projectWith([track("t1", "overlay", [item("title", "overlay", 0, 2)])]);
    const preview = evaluateClipTrim({ project, itemId: "title", edge: "right", deltaSeconds: 0.96, snap: { playheadSeconds: 3, pixelsPerSecond: 80, sticky: null } });
    expect(preview.placement.durationSeconds).toBe(3);
    expect(preview.guideSeconds).toBe(3);
  });

  it("returns no action for an unchanged edge", () => {
    const project = projectWith([track("t1", "overlay", [item("title", "overlay", 0, 2)])]);
    expect(evaluateClipTrim({ project, itemId: "title", edge: "left", deltaSeconds: 0, snap: null }).action).toBeNull();
  });
});

describe("evaluateClipRippleTrim", () => {
  it("plans a ripple trim that shifts later clips and previews every change", () => {
    const project = projectWith([
      track("t1", "overlay", [item("a", "overlay", 0, 2), item("b", "overlay", 2, 2)]),
      track("t2", "overlay", [item("c", "overlay", 3, 1)], { syncLocked: true }),
    ]);
    const preview = evaluateClipRippleTrim(project, { itemId: "a", edge: "right", deltaSeconds: 1 });
    expect(preview.blocked).toBeNull();
    expect(preview.action).toEqual({
      type: "rippleTrimItem",
      itemId: "a",
      edge: "right",
      deltaSeconds: 1,
      propagateLinked: true,
      syncLockedTrackIds: ["t2"],
    });
    expect(Object.fromEntries(preview.placements)).toEqual({
      a: { startSeconds: 0, durationSeconds: 3 },
      b: { startSeconds: 3, durationSeconds: 2 },
      c: { startSeconds: 4, durationSeconds: 1 },
    });
  });

  it("keeps at least 0.1 s and reports planner errors as blocked", () => {
    const project = projectWith([track("t1", "overlay", [item("a", "overlay", 0, 2)])]);
    expect(evaluateClipRippleTrim(project, { itemId: "a", edge: "left", deltaSeconds: 5 }).action?.deltaSeconds).toBe(1.9);
    expect(evaluateClipRippleTrim(project, { itemId: "a", edge: "right", deltaSeconds: 0 }).action).toBeNull();

    const locked = projectWith([track("t1", "overlay", [item("a", "overlay", 0, 2)], { locked: true })]);
    const blocked = evaluateClipRippleTrim(locked, { itemId: "a", edge: "right", deltaSeconds: 1 });
    expect(blocked.action).toBeNull();
    expect(blocked.blocked).toBe("Ripple trim blocked: Track t1 is locked.");
  });
});
