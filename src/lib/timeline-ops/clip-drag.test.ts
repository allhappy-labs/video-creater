import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type ProjectAction, type VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import {
  bladeSplitSeconds,
  clipMoveGroup,
  evaluateClipMove,
  planClipMoveCommit,
  type ClipMoveInput,
} from "@/lib/timeline-ops/clip-drag";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function item(id: string, kind: TimelineItemKind, startSeconds: number, durationSeconds: number, properties: Record<string, unknown> = {}): TimelineItem {
  return { id, kind, startSeconds, durationSeconds, source: { type: "text", text: id }, label: id, properties };
}

function track(id: string, kind: TrackKind, items: TimelineItem[], patch: Partial<TimelineTrack> = {}): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items, ...patch };
}

function projectWith(tracks: TimelineTrack[]): VideoProject {
  const project = fixtureProject();
  project.timeline = { durationSeconds: 60, tracks };
  return project;
}

function moveInput(project: VideoProject, patch: Partial<ClipMoveInput> & Pick<ClipMoveInput, "leadItemId">): ClipMoveInput {
  return {
    timeline: project.timeline,
    itemIds: [patch.leadItemId],
    deltaSeconds: 0,
    target: { trackId: "v1", insertIndex: null },
    duplicate: false,
    snap: null,
    ...patch,
  };
}

function applied(project: VideoProject, actions: readonly ProjectAction[]): VideoProject {
  return actions.reduce(applyProjectActionLocally, project);
}

function actionsOf(result: ReturnType<typeof planClipMoveCommit>): ProjectAction[] {
  if ("blocked" in result) throw new Error(result.blocked);
  return result.actions;
}

describe("clipMoveGroup", () => {
  it("moves the selection when the lead is selected, else only the lead, plus linked companions", () => {
    const project = projectWith([
      track("v1", "video", [item("a", "video_clip", 0, 2, { linkGroupId: "g" }), item("b", "video_clip", 3, 2)]),
      track("a1", "audio", [item("a-audio", "audio_clip", 0, 2, { linkGroupId: "g" })]),
    ]);
    expect(clipMoveGroup(project.timeline, ["b"], "a")).toEqual(["a", "a-audio"]);
    expect(clipMoveGroup(project.timeline, ["a", "b"], "b")).toEqual(["a", "b", "a-audio"]);
  });

  it("returns null when any group item sits on a locked track", () => {
    const project = projectWith([
      track("v1", "video", [item("a", "video_clip", 0, 2, { linkGroupId: "g" })]),
      track("a1", "audio", [item("a-audio", "audio_clip", 0, 2, { linkGroupId: "g" })], { locked: true }),
    ]);
    expect(clipMoveGroup(project.timeline, ["a"], "a")).toBeNull();
  });
});

describe("evaluateClipMove", () => {
  const twoClips = () => projectWith([track("v1", "video", [item("a", "video_clip", 0, 2), item("b", "video_clip", 5, 2)])]);

  it("moves a clip in time on its own track, clamped at zero", () => {
    const project = twoClips();
    const preview = evaluateClipMove(moveInput(project, { leadItemId: "a", deltaSeconds: 1.25 }));
    expect(preview.state).toBe("accepted");
    expect(preview.placements).toEqual([{ itemId: "a", trackId: "v1", startSeconds: 1.25, durationSeconds: 2 }]);
    expect(evaluateClipMove(moveInput(project, { leadItemId: "a", deltaSeconds: -9 })).placements[0]?.startSeconds).toBe(0);
  });

  it("rejects a collision with the reason at the target track", () => {
    const preview = evaluateClipMove(moveInput(twoClips(), { leadItemId: "a", deltaSeconds: 4 }));
    expect(preview.state).toBe("rejected");
    expect(preview.reason).toBe("Overlaps b");
    expect(preview.targetTrackId).toBe("v1");
    // The attempted placement stays available so the target can show the rejection.
    expect(preview.placements).toEqual([{ itemId: "a", trackId: "v1", startSeconds: 4, durationSeconds: 2 }]);
  });

  it("snaps to the playhead within 8 px and keeps the snap sticky", () => {
    const project = twoClips();
    // 80 px/s: 0.05 s is 4 px away from the playhead at 3 s.
    const first = evaluateClipMove(
      moveInput(project, { leadItemId: "b", deltaSeconds: -1.95, snap: { playheadSeconds: 3, pixelsPerSecond: 80, sticky: null } }),
    );
    expect(first.placements[0]?.startSeconds).toBe(3);
    expect(first.guideSeconds).toBe(3);
    expect(first.sticky).toEqual({ targetSeconds: 3, probeEdge: "start" });

    // 12 px away is outside the 8 px threshold but inside the 14 px sticky release.
    const sticky = evaluateClipMove(
      moveInput(project, { leadItemId: "b", deltaSeconds: -1.85, snap: { playheadSeconds: 3, pixelsPerSecond: 80, sticky: first.sticky } }),
    );
    expect(sticky.placements[0]?.startSeconds).toBe(3);
    const free = evaluateClipMove(
      moveInput(project, { leadItemId: "b", deltaSeconds: -1.85, snap: { playheadSeconds: 3, pixelsPerSecond: 80, sticky: null } }),
    );
    expect(free.placements[0]?.startSeconds).toBe(3.15);
    expect(free.guideSeconds).toBeNull();
  });

  it("snaps to the edit points of other clips but not to the dragged clip itself", () => {
    const preview = evaluateClipMove(
      moveInput(twoClips(), { leadItemId: "a", deltaSeconds: 2.95, snap: { playheadSeconds: 40, pixelsPerSecond: 80, sticky: null } }),
    );
    // The end of "a" (4.95) snaps to the start of "b" (5).
    expect(preview.placements[0]?.startSeconds).toBe(3);
    expect(preview.guideSeconds).toBe(5);
  });

  it("moves to a compatible hovered track", () => {
    const project = projectWith([track("v1", "video", [item("a", "video_clip", 0, 2)]), track("v2", "video", [])]);
    const preview = evaluateClipMove(moveInput(project, { leadItemId: "a", deltaSeconds: 1, target: { trackId: "v2", insertIndex: null } }));
    expect(preview.state).toBe("accepted");
    expect(preview.placements[0]).toMatchObject({ trackId: "v2", startSeconds: 1 });
    expect(preview.newTrack).toBeNull();
  });

  it("rejects a locked hovered track", () => {
    const project = projectWith([track("v1", "video", [item("a", "video_clip", 0, 2)]), track("v2", "video", [], { locked: true })]);
    const preview = evaluateClipMove(moveInput(project, { leadItemId: "a", target: { trackId: "v2", insertIndex: null } }));
    expect(preview.state).toBe("rejected");
    expect(preview.reason).toBe("Track locked");
    expect(preview.targetTrackId).toBe("v2");
  });

  it("creates a track in the right band when hovering an incompatible track", () => {
    const project = projectWith([track("v1", "video", [item("a", "video_clip", 0, 2)]), track("a1", "audio", [item("m", "audio_clip", 0, 2)])]);
    const preview = evaluateClipMove(moveInput(project, { leadItemId: "m", deltaSeconds: 1, target: { trackId: "v1", insertIndex: null } }));
    expect(preview.state).toBe("accepted");
    expect(preview.newTrack?.createTrack.track.kind).toBe("audio");
    // A new audio track goes to the top of the below band: right under the video band.
    expect(preview.newTrack?.insertIndex).toBe(1);
    expect(preview.targetTrackId).toBeNull();
  });

  it("creates a track between rows and commits createTrack, moveItems and the emptied track removal in one batch", () => {
    const project = projectWith([
      track("t1", "overlay", [item("title", "overlay", 0, 2)]),
      track("v1", "video", [item("a", "video_clip", 0, 2)]),
      track("v2", "video", [item("b", "video_clip", 0, 2)]),
    ]);
    const preview = evaluateClipMove(moveInput(project, { leadItemId: "b", deltaSeconds: 0.5, target: { trackId: "v1", insertIndex: 1 } }));
    expect(preview.state).toBe("accepted");
    const actions = actionsOf(planClipMoveCommit(project, preview, false));
    const newTrackId = preview.newTrack?.createTrack.track.id ?? "";
    expect(actions.map((action) => action.type)).toEqual(["createTrack", "moveItems", "removeTracks"]);
    const after = applied(project, actions);
    expect(after.timeline.tracks.map((entry) => entry.id)).toEqual(["t1", newTrackId, "v1"]);
    expect(after.timeline.tracks[1]?.items.map((entry) => [entry.id, entry.startSeconds])).toEqual([["b", 0.5]]);
  });

  it("keeps a clip that is alone on its track on that track when dropped on its own boundary", () => {
    const project = projectWith([track("v1", "video", [item("a", "video_clip", 0, 2)]), track("v2", "video", [item("b", "video_clip", 0, 2)])]);
    const preview = evaluateClipMove(moveInput(project, { leadItemId: "b", deltaSeconds: 3, target: { trackId: "v2", insertIndex: 2 } }));
    expect(preview.newTrack).toBeNull();
    expect(preview.placements[0]).toMatchObject({ trackId: "v2", startSeconds: 3 });
  });

  it("pins linked companions to their own tracks while they follow in time", () => {
    const project = projectWith([
      track("v1", "video", [item("a", "video_clip", 0, 2, { linkGroupId: "g" })]),
      track("v2", "video", []),
      track("a1", "audio", [item("a-audio", "audio_clip", 0, 2, { linkGroupId: "g" })]),
    ]);
    const preview = evaluateClipMove(
      moveInput(project, { leadItemId: "a", itemIds: ["a", "a-audio"], deltaSeconds: 2, target: { trackId: "v2", insertIndex: null } }),
    );
    expect(preview.state).toBe("accepted");
    expect(preview.placements).toEqual([
      { itemId: "a", trackId: "v2", startSeconds: 2, durationSeconds: 2 },
      { itemId: "a-audio", trackId: "a1", startSeconds: 2, durationSeconds: 2 },
    ]);
  });

  it("shifts a multi-track selection by rows and rejects destinations that do not exist", () => {
    const project = projectWith([
      track("v1", "video", [item("a", "video_clip", 0, 2)]),
      track("v2", "video", [item("b", "video_clip", 0, 2)]),
      track("v3", "video", []),
    ]);
    const down = evaluateClipMove(
      moveInput(project, { leadItemId: "a", itemIds: ["a", "b"], target: { trackId: "v2", insertIndex: null } }),
    );
    expect(down.placements.map((placement) => placement.trackId)).toEqual(["v2", "v3"]);
    const tooFar = evaluateClipMove(
      moveInput(project, { leadItemId: "a", itemIds: ["a", "b"], target: { trackId: "v3", insertIndex: null } }),
    );
    expect(tooFar.state).toBe("rejected");
    expect(tooFar.reason).toBe("Destination unavailable");
  });

  it("evaluates an Alt-drag duplicate against the originals and commits addItems with copies", () => {
    const project = twoClips();
    const onItself = evaluateClipMove(moveInput(project, { leadItemId: "a", deltaSeconds: 1, duplicate: true }));
    expect(onItself.state).toBe("rejected");
    expect(onItself.reason).toBe("Overlaps a");

    const clear = evaluateClipMove(moveInput(project, { leadItemId: "a", deltaSeconds: 2, duplicate: true }));
    expect(clear.state).toBe("accepted");
    const actions = actionsOf(planClipMoveCommit(project, clear, true));
    expect(actions).toHaveLength(1);
    expect(actions[0]).toMatchObject({ type: "addItems", targetTrackId: "v1", items: [{ id: "a-copy", label: "a copy", startSeconds: 2 }] });
  });

  it("blocks committing a rejected preview and commits nothing for an unchanged drop", () => {
    const project = twoClips();
    const rejected = evaluateClipMove(moveInput(project, { leadItemId: "a", deltaSeconds: 4 }));
    expect(planClipMoveCommit(project, rejected, false)).toEqual({ blocked: "Overlaps b" });
    const unchanged = evaluateClipMove(moveInput(project, { leadItemId: "a" }));
    expect(planClipMoveCommit(project, unchanged, false)).toEqual({ actions: [] });
  });
});

describe("bladeSplitSeconds", () => {
  const project = projectWith([track("v1", "video", [item("a", "video_clip", 0, 4), item("tiny", "video_clip", 5, 0.1)])]);

  it("splits at the pointer, rounded, snapped to the playhead when snapping is on", () => {
    expect(bladeSplitSeconds(project.timeline, "a", 1.23456, null)).toBe(1.235);
    expect(bladeSplitSeconds(project.timeline, "a", 1.96, { playheadSeconds: 2, pixelsPerSecond: 80, sticky: null })).toBe(2);
  });

  it("ignores the clip edges and clips of 0.1 s or less", () => {
    expect(bladeSplitSeconds(project.timeline, "a", 0, null)).toBeNull();
    expect(bladeSplitSeconds(project.timeline, "a", 4, null)).toBeNull();
    expect(bladeSplitSeconds(project.timeline, "tiny", 5.05, null)).toBeNull();
  });
});
