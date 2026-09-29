import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type VideoProject } from "@/lib/project";
import type { Timeline, TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import {
  emptyTrackRemovals,
  newTrackId,
  planAdjacentTrack,
  planDropTarget,
  type AssetKind,
  type DropTargetInput,
  type DropTargetPlan,
} from "@/lib/timeline-ops/dynamic-tracks";
import { orderedTracksByBand } from "@/lib/timeline-ops/track-bands";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function item(id: string, kind: TimelineItemKind, startSeconds: number, durationSeconds: number): TimelineItem {
  return {
    id,
    kind,
    startSeconds,
    durationSeconds,
    source: { type: "text", text: id },
    label: id,
    properties: {},
  };
}

function track(id: string, kind: TrackKind, items: TimelineItem[] = [], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

/** Stored order puts the video tracks first, like new projects do. */
function baseTimeline(): Timeline {
  return {
    durationSeconds: 8,
    tracks: [
      track("v1", "video", [item("clip-1", "video_clip", 0, 4)]),
      track("v2", "video", [item("clip-2", "video_clip", 2, 4)]),
      track("text-1", "overlay"),
      track("captions", "caption"),
      track("a1", "audio", [item("music", "audio_clip", 0, 8)]),
    ],
  };
}

function dropInput(overrides: Partial<DropTargetInput>): DropTargetInput {
  return {
    timeline: baseTimeline(),
    assetKind: "video",
    hoveredTrackId: null,
    insertIndex: null,
    startSeconds: 0,
    durationSeconds: 2,
    newTrackId: "track-new",
    ...overrides,
  };
}

function projectWith(timeline: Timeline): VideoProject {
  return { ...fixtureProject(), timeline };
}

/** Applies the create plan and returns the resulting top-to-bottom track ids. */
function displayOrderAfter(timeline: Timeline, plan: DropTargetPlan) {
  if (plan.kind !== "create") throw new Error(`expected a create plan, got ${plan.kind}`);
  let project = applyProjectActionLocally(projectWith(timeline), plan.createTrack);
  if (plan.reorderTrack) project = applyProjectActionLocally(project, plan.reorderTrack);
  return orderedTracksByBand(project.timeline).map((entry) => entry.id);
}

describe("planDropTarget onto an existing track", () => {
  it("uses a compatible video track when the drop time is free", () => {
    expect(planDropTarget(dropInput({ hoveredTrackId: "v1", startSeconds: 4 }))).toEqual({
      kind: "existing",
      trackId: "v1",
    });
  });

  it("creates a video track directly above the hovered one when the drop collides", () => {
    const input = dropInput({ hoveredTrackId: "v2", startSeconds: 3 });
    const plan = planDropTarget(input);
    expect(plan).toEqual({
      kind: "create",
      trackId: "track-new",
      createTrack: {
        type: "createTrack",
        track: { id: "track-new", name: "Video", kind: "video", locked: false, enabled: true, items: [] },
        afterTrackId: "v1",
      },
      reorderTrack: null,
    });
    expect(displayOrderAfter(input.timeline, plan)).toEqual(["text-1", "captions", "v1", "track-new", "v2", "a1"]);
  });

  it("adds a reorderTrack action when the hovered track is stored first and createTrack cannot precede it", () => {
    const input = dropInput({ hoveredTrackId: "v1", startSeconds: 1 });
    const plan = planDropTarget(input);
    expect(plan).toMatchObject({
      kind: "create",
      trackId: "track-new",
      createTrack: { type: "createTrack", track: { id: "track-new", kind: "video" } },
      reorderTrack: { type: "reorderTrack", trackId: "track-new", targetTrackId: "v1", placement: "before" },
    });
    expect(plan.kind === "create" && "afterTrackId" in plan.createTrack).toBe(false);
    expect(displayOrderAfter(input.timeline, plan)).toEqual(["text-1", "captions", "track-new", "v1", "v2", "a1"]);
  });
});

describe("planDropTarget creating a track", () => {
  it("creates a Text (overlay) track at the bottom of the above band when text hovers a video track", () => {
    const input = dropInput({ assetKind: "text", hoveredTrackId: "v1", startSeconds: 5 });
    const plan = planDropTarget(input);
    expect(plan).toMatchObject({
      kind: "create",
      createTrack: {
        track: { id: "track-new", name: "Text", kind: "overlay" },
        afterTrackId: "captions",
      },
      reorderTrack: null,
    });
    expect(displayOrderAfter(input.timeline, plan)).toEqual(["text-1", "captions", "track-new", "v1", "v2", "a1"]);
  });

  it("creates an Audio track at the top of the below band when audio drops between two video tracks", () => {
    const input = dropInput({ assetKind: "audio", insertIndex: 3 });
    const plan = planDropTarget(input);
    expect(plan).toMatchObject({
      kind: "create",
      createTrack: { track: { id: "track-new", name: "Audio", kind: "audio" } },
    });
    expect(displayOrderAfter(input.timeline, plan)).toEqual(["text-1", "captions", "v1", "v2", "track-new", "a1"]);
  });

  it("creates the track at the insert position when it falls inside the matching band", () => {
    const input = dropInput({ assetKind: "image", insertIndex: 3 });
    expect(displayOrderAfter(input.timeline, planDropTarget(input))).toEqual([
      "text-1",
      "captions",
      "v1",
      "track-new",
      "v2",
      "a1",
    ]);
  });

  it("maps every asset kind to its track kind and new-track name", () => {
    const assetKinds: AssetKind[] = ["video", "image", "lottie", "audio", "text", "template", "caption", "background"];
    const timeline: Timeline = { durationSeconds: 0, tracks: [] };
    expect(
      assetKinds.map((assetKind) => {
        const plan = planDropTarget(dropInput({ timeline, assetKind }));
        return plan.kind === "create" ? [assetKind, plan.createTrack.track.kind, plan.createTrack.track.name] : plan;
      }),
    ).toEqual([
      ["video", "video", "Video"],
      ["image", "video", "Video"],
      ["lottie", "video", "Video"],
      ["audio", "audio", "Audio"],
      ["text", "overlay", "Text"],
      ["template", "overlay", "Text"],
      ["caption", "caption", "Captions"],
      ["background", "hyperframe_scene", "Graphics"],
    ]);
  });

  it("clamps an insert position outside the item's band to the nearest edge of that band", () => {
    const input = dropInput({ assetKind: "background", insertIndex: 5 });
    const plan = planDropTarget(input);
    expect(plan).toMatchObject({ kind: "create", createTrack: { afterTrackId: "captions" } });
    expect(displayOrderAfter(input.timeline, plan)).toEqual(["text-1", "captions", "track-new", "v1", "v2", "a1"]);
  });

  it("uses a free compatible caption track and appends a band that does not exist yet", () => {
    expect(planDropTarget(dropInput({ assetKind: "caption", hoveredTrackId: "captions" }))).toEqual({
      kind: "existing",
      trackId: "captions",
    });
    const timeline: Timeline = { durationSeconds: 4, tracks: [track("v1", "video")] };
    const plan = planDropTarget(dropInput({ timeline, assetKind: "lottie", hoveredTrackId: null }));
    expect(plan).toMatchObject({ kind: "create", createTrack: { afterTrackId: "v1" }, reorderTrack: null });
    const audio = planDropTarget(dropInput({ timeline, assetKind: "audio", insertIndex: 0 }));
    expect(audio).toMatchObject({ kind: "create", reorderTrack: null });
    expect(audio.kind === "create" && "afterTrackId" in audio.createTrack).toBe(false);
  });
});

describe("planDropTarget invalid drops", () => {
  it("rejects a drop onto a locked track, compatible or not", () => {
    const timeline = baseTimeline();
    timeline.tracks = timeline.tracks.map((entry) => (entry.id === "v1" ? { ...entry, locked: true } : entry));
    expect(planDropTarget(dropInput({ timeline, hoveredTrackId: "v1", startSeconds: 6 }))).toEqual({
      kind: "invalid",
      reason: "Track is locked",
    });
    expect(planDropTarget(dropInput({ timeline, assetKind: "audio", hoveredTrackId: "v1" }))).toEqual({
      kind: "invalid",
      reason: "Track is locked",
    });
  });

  it("rejects a new track id that already exists", () => {
    expect(planDropTarget(dropInput({ newTrackId: "v2", insertIndex: 0 }))).toEqual({
      kind: "invalid",
      reason: "Track v2 already exists",
    });
  });
});

describe("emptyTrackRemovals", () => {
  function removeItems(timeline: Timeline, itemIds: string[]): Timeline {
    return {
      ...timeline,
      tracks: timeline.tracks.map((entry) => ({
        ...entry,
        items: entry.items.filter((candidate) => !itemIds.includes(candidate.id)),
      })),
    };
  }

  it("returns null when nothing becomes empty", () => {
    const before = baseTimeline();
    expect(emptyTrackRemovals(before, before)).toBeNull();
  });

  it("removes only the tracks that this edit emptied, never ones that were already empty", () => {
    const before = baseTimeline();
    expect(emptyTrackRemovals(before, removeItems(before, ["clip-2", "music"]))).toEqual({
      type: "removeTracks",
      trackIds: ["v2", "a1"],
    });
  });

  it("never removes the last video track", () => {
    const before = baseTimeline();
    expect(emptyTrackRemovals(before, removeItems(before, ["clip-1", "clip-2"]))).toEqual({
      type: "removeTracks",
      trackIds: ["v2"],
    });
    const single: Timeline = { durationSeconds: 4, tracks: [track("v1", "video", [item("clip-1", "video_clip", 0, 4)])] };
    expect(emptyTrackRemovals(single, removeItems(single, ["clip-1"]))).toBeNull();
  });

  it("keeps an emptied locked track because removeTracks rejects locked tracks", () => {
    const before = baseTimeline();
    const after = removeItems(before, ["music", "clip-2"]);
    after.tracks = after.tracks.map((entry) => (entry.id === "a1" ? { ...entry, locked: true } : entry));
    expect(emptyTrackRemovals(before, after)).toEqual({ type: "removeTracks", trackIds: ["v2"] });
  });
});

describe("planAdjacentTrack", () => {
  function displayIdsAfter(timeline: Timeline, result: ReturnType<typeof planAdjacentTrack>): string[] {
    if ("blocked" in result) throw new Error(result.blocked);
    const project = result.actions.reduce(applyProjectActionLocally, projectWith(timeline));
    return orderedTracksByBand(project.timeline).map((entry) => entry.id);
  }

  it("adds an empty track of the same kind above the first stored track with a reorder", () => {
    const timeline = baseTimeline();
    const result = planAdjacentTrack(timeline, "v1", "above");
    expect(result).toEqual({
      actions: [
        { type: "createTrack", track: { id: "track-video-6", name: "Video", kind: "video", locked: false, enabled: true, items: [] } },
        { type: "reorderTrack", trackId: "track-video-6", targetTrackId: "v1", placement: "before" },
      ],
    });
    expect(displayIdsAfter(timeline, result)).toEqual(["text-1", "captions", "track-video-6", "v1", "v2", "a1"]);
  });

  it("adds tracks below a track and above a later track in its band", () => {
    const timeline = baseTimeline();
    expect(displayIdsAfter(timeline, planAdjacentTrack(timeline, "v1", "below"))).toEqual(["text-1", "captions", "v1", "track-video-6", "v2", "a1"]);
    expect(displayIdsAfter(timeline, planAdjacentTrack(timeline, "captions", "above"))).toEqual(["text-1", "track-caption-6", "captions", "v1", "v2", "a1"]);
    expect(displayIdsAfter(timeline, planAdjacentTrack(timeline, "a1", "below"))).toEqual(["text-1", "captions", "v1", "v2", "a1", "track-audio-6"]);
  });

  it("blocks a missing track and picks unused track ids", () => {
    expect(planAdjacentTrack(baseTimeline(), "missing", "above")).toEqual({ blocked: "That track no longer exists." });
    const timeline = baseTimeline();
    timeline.tracks.push(track("track-audio-6", "audio"));
    expect(newTrackId(timeline, "audio")).toBe("track-audio-7");
  });
});
