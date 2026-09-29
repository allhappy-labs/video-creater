import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type ProjectAction, type VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import {
  decomposeNested,
  defaultSplitItemId,
  deleteGapAt,
  deleteItems,
  duplicateItems,
  linkItems,
  nudgeItems,
  rippleDeleteItems,
  setItemDuration,
  setItemReverse,
  setItemSpeed,
  splitAtPlayhead,
  unlinkItems,
  type CommandResult,
} from "@/lib/timeline-ops/clip-commands";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function item(
  id: string,
  kind: TimelineItemKind,
  startSeconds: number,
  durationSeconds: number,
  properties: Record<string, unknown> = {},
  source: TimelineItem["source"] = { type: "text", text: id },
): TimelineItem {
  return { id, kind, startSeconds, durationSeconds, source, label: id, properties };
}

function mediaItem(
  id: string,
  kind: TimelineItemKind,
  startSeconds: number,
  durationSeconds: number,
  properties: Record<string, unknown> = {},
): TimelineItem {
  // fixtureProject() media "media-1" is a 4 second video.
  return item(id, kind, startSeconds, durationSeconds, properties, { type: "media", mediaId: "media-1" });
}

function track(
  id: string,
  kind: TrackKind,
  items: TimelineItem[] = [],
  options: { locked?: boolean; syncLocked?: boolean } = {},
): TimelineTrack {
  return {
    id,
    name: id,
    kind,
    locked: options.locked ?? false,
    ...(options.syncLocked ? { syncLocked: true } : {}),
    enabled: true,
    items,
  };
}

function projectWith(tracks: TimelineTrack[]): VideoProject {
  const durationSeconds = Math.max(
    0,
    ...tracks.flatMap((candidate) => candidate.items.map((entry) => entry.startSeconds + entry.durationSeconds)),
  );
  return { ...fixtureProject(), timeline: { durationSeconds, tracks } };
}

function actionsOf(result: CommandResult): ProjectAction[] {
  if ("blocked" in result) throw new Error(`Unexpectedly blocked: ${result.blocked}`);
  return result.actions;
}

function applyAll(project: VideoProject, actions: readonly ProjectAction[]) {
  return actions.reduce(applyProjectActionLocally, project);
}

function placement(project: VideoProject, itemId: string) {
  for (const candidate of project.timeline.tracks) {
    const found = candidate.items.find((entry) => entry.id === itemId);
    if (found) return { trackId: candidate.id, start: found.startSeconds, duration: found.durationSeconds };
  }
  return null;
}

describe("splitAtPlayhead", () => {
  it("splits a clip the playhead is strictly inside", () => {
    const project = projectWith([track("v1", "video", [mediaItem("clip-a", "video_clip", 0, 4)])]);
    expect(splitAtPlayhead(project, ["clip-a"], 1.5, (itemId) => `${itemId}-right`)).toEqual({
      actions: [{ type: "splitItems", splits: [{ itemId: "clip-a", newItemId: "clip-a-right", splitSeconds: 1.5 }] }],
    });
  });

  it("names right halves like the legacy editor by default", () => {
    expect(defaultSplitItemId("clip-a", 1.2345)).toBe("clip-a-split-1235");
  });

  it("is blocked when the playhead sits on a clip boundary", () => {
    const project = projectWith([track("v1", "video", [mediaItem("clip-a", "video_clip", 1, 3)])]);
    const blocked = { blocked: "Move the playhead over the selected clip to split." };
    expect(splitAtPlayhead(project, ["clip-a"], 1, (id) => `${id}-r`)).toEqual(blocked);
    expect(splitAtPlayhead(project, ["clip-a"], 4, (id) => `${id}-r`)).toEqual(blocked);
    expect(splitAtPlayhead(project, [], 2, (id) => `${id}-r`)).toEqual({ blocked: "Select a clip to split." });
  });

  it("skips clips on locked tracks, and is blocked when every clip is locked", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("clip-a", "video_clip", 0, 4)], { locked: true }),
      track("a1", "audio", [item("music", "audio_clip", 0, 4)]),
    ]);
    expect(actionsOf(splitAtPlayhead(project, ["clip-a", "music"], 2, (id) => `${id}-r`))).toEqual([
      { type: "splitItems", splits: [{ itemId: "music", newItemId: "music-r", splitSeconds: 2 }] },
    ]);
    expect(splitAtPlayhead(project, ["clip-a"], 2, (id) => `${id}-r`)).toEqual({
      blocked: "Unlock the selected tracks to split these clips.",
    });
  });

  it("splits linked clips together and re-links the right halves", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("clip-a", "video_clip", 0, 4, { linkGroupId: "link-1" })]),
      track("a1", "audio", [item("voice", "audio_clip", 0, 4, { linkGroupId: "link-1" })]),
    ]);
    const actions = actionsOf(
      splitAtPlayhead(project, ["clip-a", "voice"], 2, (id) => defaultSplitItemId(id, 2)),
    );
    expect(actions).toEqual([
      {
        type: "splitItems",
        splits: [
          { itemId: "clip-a", newItemId: "clip-a-split-2000", splitSeconds: 2 },
          { itemId: "voice", newItemId: "voice-split-2000", splitSeconds: 2 },
        ],
      },
      {
        type: "updateItemProperties",
        updates: [
          { itemId: "clip-a-split-2000", set: { linkGroupId: "link-split-2000" }, remove: [] },
          { itemId: "voice-split-2000", set: { linkGroupId: "link-split-2000" }, remove: [] },
        ],
      },
    ]);
    const next = applyAll(project, actions);
    const groups = next.timeline.tracks.flatMap((candidate) =>
      candidate.items.map((entry) => [entry.id, entry.properties.linkGroupId]),
    );
    expect(groups).toEqual([
      ["clip-a", "link-1"],
      ["clip-a-split-2000", "link-split-2000"],
      ["voice", "link-1"],
      ["voice-split-2000", "link-split-2000"],
    ]);
  });

  it("keeps new clip and link group ids unique", () => {
    const project = projectWith([
      track("v1", "video", [
        mediaItem("clip-a", "video_clip", 0, 4, { linkGroupId: "link-1" }),
        mediaItem("clip-a-split-2000", "video_clip", 5, 1, { linkGroupId: "link-split-2000" }),
      ]),
      track("a1", "audio", [item("voice", "audio_clip", 0, 4, { linkGroupId: "link-1" })]),
    ]);
    const actions = actionsOf(
      splitAtPlayhead(project, ["clip-a", "voice"], 2, (id) => defaultSplitItemId(id, 2)),
    );
    expect(actions[0]).toMatchObject({
      splits: [{ newItemId: "clip-a-split-2000-2" }, { newItemId: "voice-split-2000" }],
    });
    expect(actions[1]).toMatchObject({
      updates: [{ set: { linkGroupId: "link-split-2000-2" } }, { set: { linkGroupId: "link-split-2000-2" } }],
    });
  });
});

describe("deleteItems", () => {
  it("removes the clips and the tracks they leave empty in one batch", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("clip-a", "video_clip", 0, 2)]),
      track("v2", "video", [mediaItem("clip-b", "video_clip", 0, 2)]),
    ]);
    expect(deleteItems(project, ["clip-b"])).toEqual({
      actions: [
        { type: "removeItems", itemIds: ["clip-b"] },
        { type: "removeTracks", trackIds: ["v2"] },
      ],
    });
  });

  it("only removes items when no track becomes empty", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("clip-a", "video_clip", 0, 2), mediaItem("clip-b", "video_clip", 2, 2)]),
    ]);
    expect(deleteItems(project, ["clip-a", "missing"])).toEqual({
      actions: [{ type: "removeItems", itemIds: ["clip-a"] }],
    });
  });

  it("is blocked without a selection or on locked tracks", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("clip-a", "video_clip", 0, 2)], { locked: true }),
    ]);
    expect(deleteItems(project, [])).toEqual({ blocked: "Select a clip to delete." });
    expect(deleteItems(project, ["clip-a"])).toEqual({
      blocked: "Unlock the selected tracks to remove these clips.",
    });
  });
});

describe("rippleDeleteItems", () => {
  it("closes each clip on its own track and the selection union on sync-locked tracks", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2), mediaItem("b", "video_clip", 3, 2)]),
      track("v2", "video", [mediaItem("c", "video_clip", 1, 2)]),
      track("text", "overlay", [item("t", "overlay", 6, 1)], { syncLocked: true }),
      track("music", "audio", [item("m", "audio_clip", 0, 8)], { locked: true, syncLocked: true }),
    ]);
    const actions = actionsOf(rippleDeleteItems(project, ["a", "c"]));
    expect(actions).toEqual([
      {
        type: "rippleDeleteRanges",
        ranges: [
          { startSeconds: 0, endSeconds: 2, trackIds: ["v1"] },
          { startSeconds: 0, endSeconds: 3, trackIds: ["text"] },
          { startSeconds: 1, endSeconds: 3, trackIds: ["v2"] },
        ],
      },
      { type: "removeTracks", trackIds: ["v2"] },
    ]);
    const next = applyAll(project, actions);
    expect(placement(next, "b")).toEqual({ trackId: "v1", start: 1, duration: 2 });
    expect(placement(next, "t")).toEqual({ trackId: "text", start: 3, duration: 1 });
    expect(placement(next, "m")).toEqual({ trackId: "music", start: 0, duration: 8 });
  });

  it("ripples the linked tracks of a single clip", () => {
    const project = projectWith([
      track("v1", "video", [
        mediaItem("a", "video_clip", 0, 2, { linkGroupId: "link-1" }),
        mediaItem("b", "video_clip", 2, 2),
      ]),
      track("a1", "audio", [
        item("voice", "audio_clip", 0, 2, { linkGroupId: "link-1" }),
        item("voice-2", "audio_clip", 2, 2),
      ]),
    ]);
    expect(rippleDeleteItems(project, ["a"])).toEqual({
      actions: [{ type: "rippleDeleteRanges", ranges: [{ startSeconds: 0, endSeconds: 2, trackIds: ["v1", "a1"] }] }],
    });
  });

  it("merges overlapping clips on one track into one range", () => {
    const project = projectWith([
      track("v1", "video", [
        mediaItem("a", "video_clip", 0, 2),
        mediaItem("b", "video_clip", 2, 1),
        mediaItem("c", "video_clip", 5, 1),
      ]),
    ]);
    expect(rippleDeleteItems(project, ["b", "a"])).toEqual({
      actions: [{ type: "rippleDeleteRanges", ranges: [{ startSeconds: 0, endSeconds: 3, trackIds: ["v1"] }] }],
    });
  });

  it("is blocked on locked tracks", () => {
    const project = projectWith([track("v1", "video", [mediaItem("a", "video_clip", 0, 2)], { locked: true })]);
    expect(rippleDeleteItems(project, ["a"])).toEqual({
      blocked: "Unlock the selected tracks to ripple delete these clips.",
    });
    expect(rippleDeleteItems(project, [])).toEqual({ blocked: "Select a clip to ripple delete." });
  });
});

describe("deleteGapAt", () => {
  it("shifts later clips on the gap track and on sync-locked tracks", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2), mediaItem("b", "video_clip", 4, 2)]),
      track("v2", "video", [mediaItem("c", "video_clip", 5, 1)], { syncLocked: true }),
      track("a1", "audio", [item("m", "audio_clip", 5, 1)]),
    ]);
    expect(deleteGapAt(project, "v1", 3)).toEqual({
      actions: [
        {
          type: "moveItems",
          moves: [
            { itemId: "b", targetTrackId: "v1", startSeconds: 2 },
            { itemId: "c", targetTrackId: "v2", startSeconds: 3 },
          ],
        },
      ],
    });
  });

  it("is blocked when there is no gap under the playhead", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2), mediaItem("b", "video_clip", 4, 2)]),
    ]);
    const blocked = { blocked: "Move the playhead into an empty gap to delete it." };
    expect(deleteGapAt(project, "v1", 1)).toEqual(blocked);
    expect(deleteGapAt(project, "v1", 7)).toEqual(blocked);
    expect(deleteGapAt(project, "missing", 3)).toEqual({ blocked: "That track no longer exists." });
  });

  it("is blocked when closing the gap would overlap a clip on a sync-locked track", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2), mediaItem("b", "video_clip", 4, 2)]),
      track("v2", "video", [mediaItem("c", "video_clip", 1, 2), mediaItem("d", "video_clip", 4, 1)], {
        syncLocked: true,
      }),
    ]);
    expect(deleteGapAt(project, "v1", 3)).toEqual({ blocked: "Can't close this gap. Overlaps c." });
  });
});

describe("nudgeItems", () => {
  it("moves clips by whole frames at the project frame rate", () => {
    const project = projectWith([track("v1", "video", [mediaItem("a", "video_clip", 1, 1)])]);
    expect(nudgeItems(project, ["a"], 1)).toEqual({
      actions: [{ type: "moveItems", moves: [{ itemId: "a", targetTrackId: "v1", startSeconds: 1.042 }] }],
    });
    expect(nudgeItems(project, ["a"], -1)).toMatchObject({ actions: [{ moves: [{ startSeconds: 0.958 }] }] });

    const at30 = { ...project, renderSettings: { ...project.renderSettings, fps: 30 } };
    expect(nudgeItems(at30, ["a"], 1)).toMatchObject({ actions: [{ moves: [{ startSeconds: 1.033 }] }] });
    expect(nudgeItems(at30, ["a"], -1)).toMatchObject({ actions: [{ moves: [{ startSeconds: 0.967 }] }] });
  });

  it("moves a multi-selection together and stops at the timeline start", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0.02, 1)]),
      track("a1", "audio", [item("m", "audio_clip", 2, 1)]),
    ]);
    expect(nudgeItems(project, ["a", "m"], -1)).toEqual({
      actions: [
        {
          type: "moveItems",
          moves: [
            { itemId: "a", targetTrackId: "v1", startSeconds: 0 },
            { itemId: "m", targetTrackId: "a1", startSeconds: 1.98 },
          ],
        },
      ],
    });
    const atStart = projectWith([track("v1", "video", [mediaItem("a", "video_clip", 0, 1)])]);
    expect(nudgeItems(atStart, ["a"], -1)).toEqual({
      blocked: "The selected clips are already at the start of the timeline.",
    });
  });

  it("is blocked with the evaluator reason when the nudge collides", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 1), mediaItem("b", "video_clip", 1, 1)]),
    ]);
    expect(nudgeItems(project, ["a"], 1)).toEqual({ blocked: "Overlaps b" });
  });

  it("is blocked on locked tracks, without a selection, or for zero frames", () => {
    const project = projectWith([track("v1", "video", [mediaItem("a", "video_clip", 1, 1)], { locked: true })]);
    expect(nudgeItems(project, ["a"], 1)).toEqual({ blocked: "Unlock the selected tracks to nudge these clips." });
    expect(nudgeItems(project, [], 1)).toEqual({ blocked: "Select a clip to nudge." });
    const unlocked = projectWith([track("v1", "video", [mediaItem("a", "video_clip", 1, 1)])]);
    expect(nudgeItems(unlocked, ["a"], 0)).toEqual({ blocked: "Nudge by at least one frame." });
  });
});

describe("duplicateItems", () => {
  it("places copies after the selection and gives linked copies a new shared group", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2, { linkGroupId: "link-1" })]),
      track("a1", "audio", [
        item("voice", "audio_clip", 0, 2, { linkGroupId: "link-1" }),
        item("voice-copy", "audio_clip", 6, 1),
      ]),
    ]);
    expect(duplicateItems(project, ["a", "voice"])).toEqual({
      actions: [
        {
          type: "addItems",
          targetTrackId: "v1",
          items: [
            {
              ...mediaItem("a-copy", "video_clip", 2, 2, { linkGroupId: "link-copy-a-copy" }),
              label: "a copy",
            },
          ],
        },
        {
          type: "addItems",
          targetTrackId: "a1",
          items: [
            {
              ...item("voice-copy-2", "audio_clip", 2, 2, { linkGroupId: "link-copy-a-copy" }),
              source: { type: "text", text: "voice" },
              label: "voice copy",
            },
          ],
        },
      ],
    });
  });

  it("drops the link group when only one member is duplicated", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 1, 2, { linkGroupId: "link-1" })]),
    ]);
    expect(duplicateItems(project, ["a"])).toMatchObject({
      actions: [{ items: [{ id: "a-copy", startSeconds: 3, properties: {} }] }],
    });
    expect(duplicateItems(projectWith([track("v1", "video", [mediaItem("a", "video_clip", 1, 2)], { locked: true })]), ["a"]))
      .toEqual({ blocked: "Unlock the selected tracks to duplicate these clips." });
  });
});

describe("setItemDuration", () => {
  it("trims the source out point and clamps to the source media duration", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2, { sourceIn: 1, sourceOut: 3 })]),
    ]);
    expect(setItemDuration(project, "a", 10)).toEqual({
      actions: [{ type: "trimItems", trims: [{ itemId: "a", startSeconds: 0, durationSeconds: 3, sourceIn: 1, sourceOut: 4 }] }],
    });
    expect(setItemDuration(project, "a", 1.5)).toEqual({
      actions: [{ type: "trimItems", trims: [{ itemId: "a", startSeconds: 0, durationSeconds: 1.5, sourceIn: 1, sourceOut: 2.5 }] }],
    });
  });

  it("resizes clips without a source range, clamping media clips to the media length", () => {
    const text = projectWith([track("t1", "overlay", [item("title", "overlay", 0, 2)])]);
    expect(setItemDuration(text, "title", 5)).toEqual({
      actions: [{ type: "resizeItems", resizes: [{ itemId: "title", durationSeconds: 5 }] }],
    });
    const media = projectWith([track("v1", "video", [mediaItem("a", "video_clip", 0, 2)])]);
    expect(setItemDuration(media, "a", 10)).toEqual({
      actions: [{ type: "resizeItems", resizes: [{ itemId: "a", durationSeconds: 4 }] }],
    });
    expect(setItemDuration(text, "title", 2)).toEqual({ actions: [] });
  });

  it("is blocked by collisions, invalid durations and locked tracks", () => {
    const project = projectWith([
      track("t1", "overlay", [item("title", "overlay", 0, 2), item("next", "overlay", 3, 1)]),
      track("t2", "overlay", [item("locked", "overlay", 0, 2)], { locked: true }),
    ]);
    expect(setItemDuration(project, "title", 5)).toEqual({
      blocked: "Overlaps next. Make room after this clip first.",
    });
    expect(setItemDuration(project, "title", 0)).toEqual({ blocked: "Enter a duration longer than zero." });
    expect(setItemDuration(project, "missing", 1)).toEqual({ blocked: "That clip no longer exists." });
    expect(setItemDuration(project, "locked", 1)).toEqual({
      blocked: "Unlock the track to change this clip's duration.",
    });
  });
});

describe("setItemSpeed", () => {
  it("changes speed and rescales the duration in one batch", () => {
    const project = projectWith([
      track("v1", "video", [
        mediaItem("a", "video_clip", 0, 4, { sourceIn: 0, sourceOut: 4 }),
        mediaItem("b", "video_clip", 10, 2, { sourceIn: 0, sourceOut: 4, speed: 2 }),
      ]),
    ]);
    expect(setItemSpeed(project, "a", 2)).toEqual({
      actions: [
        { type: "updateVisualClipSpeed", itemId: "a", speed: 2 },
        { type: "resizeItems", resizes: [{ itemId: "a", durationSeconds: 2 }] },
      ],
    });
    expect(setItemSpeed(project, "b", 0.5)).toMatchObject({
      actions: [{ speed: 0.5 }, { resizes: [{ itemId: "b", durationSeconds: 8 }] }],
    });
    expect(setItemSpeed(project, "b", 2)).toEqual({ actions: [] });
  });

  it("is available for visual and audio clips inside 0.1x to 8x", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 4), mediaItem("b", "video_clip", 4, 1)]),
      track("a1", "audio", [item("music", "audio_clip", 0, 4)]),
    ]);
    expect(setItemSpeed(project, "music", 2)).toEqual({
      actions: [
        { type: "updateAudioClipSpeed", itemId: "music", speed: 2 },
        { type: "resizeItems", resizes: [{ itemId: "music", durationSeconds: 2 }] },
      ],
    });
    const outOfRange = { blocked: "Enter a speed between 0.1x and 8x." };
    expect(setItemSpeed(project, "a", 0.05)).toEqual(outOfRange);
    expect(setItemSpeed(project, "a", 9)).toEqual(outOfRange);
    expect(setItemSpeed(project, "a", Number.NaN)).toEqual(outOfRange);
    expect(setItemSpeed(project, "a", 0.5)).toEqual({ blocked: "Overlaps b. Make room after this clip first." });
  });

  it("retimes linked video and audio partners in one batch", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 4, { linkGroupId: "link-1" })]),
      track("a1", "audio", [mediaItem("a-sound", "audio_clip", 0, 4, { linkGroupId: "link-1" })]),
      track("t1", "overlay", [item("title", "overlay", 0, 4, { linkGroupId: "link-1" })]),
    ]);
    expect(setItemSpeed(project, "a-sound", 2)).toEqual({
      actions: [
        { type: "updateAudioClipSpeed", itemId: "a-sound", speed: 2 },
        { type: "updateVisualClipSpeed", itemId: "a", speed: 2 },
        { type: "resizeItems", resizes: [{ itemId: "a-sound", durationSeconds: 2 }, { itemId: "a", durationSeconds: 2 }] },
      ],
    });
  });

  it("blocks when a linked partner has no room for its new duration", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2, { linkGroupId: "link-1" }), mediaItem("b", "video_clip", 3, 1)]),
      track("a1", "audio", [mediaItem("a-sound", "audio_clip", 0, 2, { linkGroupId: "link-1" })]),
    ]);
    expect(setItemSpeed(project, "a-sound", 0.5)).toEqual({ blocked: "Overlaps b. Make room after this clip first." });
  });

  it("stays unavailable for captions", () => {
    const project = projectWith([track("c1", "caption", [item("caption", "caption", 0, 4)])]);
    expect(setItemSpeed(project, "caption", 2)).toEqual({ blocked: "Speed is available for video, image and audio clips." });
  });
});

describe("setItemReverse", () => {
  it("reverses the clip and its linked video or audio partners in one batch", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 4, { linkGroupId: "link-1" })]),
      track("a1", "audio", [mediaItem("a-sound", "audio_clip", 0, 4, { linkGroupId: "link-1" })]),
      track("t1", "overlay", [item("title", "overlay", 0, 4, { linkGroupId: "link-1" })]),
    ]);
    const actions = actionsOf(setItemReverse(project, "a", true));
    expect(actions).toEqual([
      { type: "updateClipReverse", itemId: "a", reverse: true },
      { type: "updateClipReverse", itemId: "a-sound", reverse: true },
    ]);
    const reversed = applyAll(project, actions);
    expect(setItemReverse(reversed, "a-sound", false)).toEqual({
      actions: [
        { type: "updateClipReverse", itemId: "a-sound", reverse: false },
        { type: "updateClipReverse", itemId: "a", reverse: false },
      ],
    });
    expect(setItemReverse(reversed, "a", true)).toEqual({ actions: [] });
  });

  it("only reverses video clips of video media and media audio clips", () => {
    const project = projectWith([
      track("v1", "video", [
        mediaItem("a", "video_clip", 0, 4, { linkGroupId: "link-1" }),
        item("still", "image_clip", 4, 2, { linkGroupId: "link-1" }, { type: "media", mediaId: "media-1" }),
      ]),
      track("c1", "caption", [item("caption", "caption", 0, 4)]),
    ]);
    expect(setItemReverse(project, "a", true)).toEqual({ actions: [{ type: "updateClipReverse", itemId: "a", reverse: true }] });
    const unavailable = { blocked: "Reverse is available for video and audio clips." };
    expect(setItemReverse(project, "still", true)).toEqual(unavailable);
    expect(setItemReverse(project, "caption", true)).toEqual(unavailable);
    expect(setItemReverse(project, "missing", true)).toEqual({ blocked: "That clip no longer exists." });
  });

  it("blocks on a locked track or a locked linked partner", () => {
    const project = projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 4, { linkGroupId: "link-1" })], { locked: true }),
      track("a1", "audio", [mediaItem("a-sound", "audio_clip", 0, 4, { linkGroupId: "link-1" })]),
    ]);
    expect(setItemReverse(project, "a", true)).toEqual({ blocked: "Unlock the track to reverse this clip." });
    expect(setItemReverse(project, "a-sound", true)).toEqual({ blocked: "Unlock the linked clip's track to reverse it." });
  });
});

describe("linkItems and unlinkItems", () => {
  const project = () =>
    projectWith([
      track("v1", "video", [mediaItem("a", "video_clip", 0, 2, { linkGroupId: "link-1" })]),
      track("a1", "audio", [item("voice", "audio_clip", 0, 2, { linkGroupId: "link-1" }), item("m", "audio_clip", 3, 1)]),
    ]);

  it("links two or more clips", () => {
    expect(linkItems(project(), ["a", "m", "a"], "link-2")).toEqual({
      actions: [{ type: "linkItems", itemIds: ["a", "m"], linkGroupId: "link-2" }],
    });
    expect(linkItems(project(), ["a"], "link-2")).toEqual({ blocked: "Select at least two clips to link." });
  });

  it("unlinks clips when any of them is linked", () => {
    expect(unlinkItems(project(), ["a", "voice"])).toEqual({
      actions: [{ type: "unlinkItems", itemIds: ["a", "voice"] }],
    });
    expect(unlinkItems(project(), ["m"])).toEqual({ blocked: "The selected clips are not linked." });
  });
});

describe("decomposeNested", () => {
  function nestedProject(): VideoProject {
    const project = projectWith([
      track("v1", "video", [item("seq", "video_clip", 0, 4, {}, { type: "timeline", timelineId: "timeline-2" })]),
      track("v2", "video", [mediaItem("a", "video_clip", 0, 1)]),
    ]);
    project.activeTimelineId = "main";
    project.timelines = [
      { id: "main", name: "Main", timeline: project.timeline },
      {
        id: "timeline-2",
        name: "Intro",
        timeline: { durationSeconds: 2, tracks: [track("nv", "video", [mediaItem("n", "video_clip", 0, 2)])] },
      },
    ];
    return project;
  }

  it("decomposes nested timeline items and removes the emptied track", () => {
    expect(decomposeNested(nestedProject(), "seq")).toEqual({
      actions: [
        { type: "decomposeTimelineItem", itemId: "seq" },
        { type: "removeTracks", trackIds: ["v1"] },
      ],
    });
  });

  it("applies only to items whose source is a timeline", () => {
    expect(decomposeNested(nestedProject(), "a")).toEqual({
      blocked: "Select one nested timeline sequence to decompose.",
    });
  });
});
