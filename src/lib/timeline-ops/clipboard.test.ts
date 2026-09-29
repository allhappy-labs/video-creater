import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type ProjectAction, type VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { copyItems, pastePlan, type TimelineClipboard } from "@/lib/timeline-ops/clipboard";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function item(
  id: string,
  kind: TimelineItemKind,
  startSeconds: number,
  durationSeconds: number,
  properties: Record<string, unknown> = {},
): TimelineItem {
  return { id, kind, startSeconds, durationSeconds, source: { type: "text", text: id }, label: id, properties };
}

function track(id: string, kind: TrackKind, items: TimelineItem[] = [], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

function projectWith(tracks: TimelineTrack[]): VideoProject {
  return { ...fixtureProject(), timeline: { durationSeconds: 10, tracks } };
}

function actionsOf(result: CommandResult): ProjectAction[] {
  if ("blocked" in result) throw new Error(`Unexpectedly blocked: ${result.blocked}`);
  return result.actions;
}

function requireClipboard(clipboard: TimelineClipboard | null): TimelineClipboard {
  if (!clipboard) throw new Error("Expected a clipboard");
  return clipboard;
}

/** Stored order is video, text, audio; band order puts text above video. */
function baseProject() {
  return projectWith([
    track("v1", "video", [item("a", "video_clip", 1, 2, { linkGroupId: "link-1" }), item("b", "video_clip", 4, 1)]),
    track("text", "overlay", [item("t", "overlay", 2, 1)]),
    track("a1", "audio", [item("m", "audio_clip", 1.5, 1, { linkGroupId: "link-1" })]),
  ]);
}

describe("copyItems", () => {
  it("stores clones with track and start offsets relative to the first clip in display order", () => {
    const project = baseProject();
    const clipboard = requireClipboard(copyItems(project.timeline, ["m", "a", "t"]));
    expect(clipboard.durationSeconds).toBe(2);
    expect(
      clipboard.entries.map(({ trackId, trackOffset, startOffset, item: entry }) => [trackId, trackOffset, startOffset, entry.id]),
    ).toEqual([
      ["text", 0, 1, "t"],
      ["v1", 1, 0, "a"],
      ["a1", 2, 0.5, "m"],
    ]);
    const copied = clipboard.entries[1]?.item;
    expect(copied).toEqual(project.timeline.tracks[0]?.items[0]);
    expect(copied?.properties).not.toBe(project.timeline.tracks[0]?.items[0]?.properties);
  });

  it("returns null when nothing matches", () => {
    expect(copyItems(baseProject().timeline, ["missing"])).toBeNull();
  });
});

describe("pastePlan", () => {
  it("pastes at the playhead onto the original tracks with injected ids and remapped links", () => {
    const project = baseProject();
    const clipboard = requireClipboard(copyItems(project.timeline, ["a", "m"]));
    const actions = actionsOf(pastePlan(project, clipboard, 6, "paste", (id) => `${id}-pasted`));
    expect(actions).toEqual([
      {
        type: "addItems",
        targetTrackId: "v1",
        items: [
          {
            ...item("a-pasted", "video_clip", 6, 2, { linkGroupId: "link-paste-a-pasted" }),
            source: { type: "text", text: "a" },
            label: "a copy",
          },
        ],
      },
      {
        type: "addItems",
        targetTrackId: "a1",
        items: [
          {
            ...item("m-pasted", "audio_clip", 6.5, 1, { linkGroupId: "link-paste-a-pasted" }),
            source: { type: "text", text: "m" },
            label: "m copy",
          },
        ],
      },
    ]);
  });

  it("uses duplicate ids by default and drops a lone link group", () => {
    const project = baseProject();
    const clipboard = requireClipboard(copyItems(project.timeline, ["a"]));
    expect(pastePlan(project, clipboard, 7, "paste")).toMatchObject({
      actions: [{ type: "addItems", targetTrackId: "v1", items: [{ id: "a-copy", startSeconds: 7, properties: {} }] }],
    });
  });

  it("paste insert uses insertItems and ripples later clips", () => {
    const project = baseProject();
    const clipboard = requireClipboard(copyItems(project.timeline, ["b"]));
    const actions = actionsOf(pastePlan(project, clipboard, 1, "insert", (id) => `${id}-inserted`));
    expect(actions).toEqual([
      {
        type: "insertItems",
        targetTrackId: "v1",
        insertSeconds: 1,
        items: [{ ...item("b-inserted", "video_clip", 0, 1), source: { type: "text", text: "b" }, label: "b insert" }],
      },
    ]);
    const next = actions.reduce(applyProjectActionLocally, project);
    expect(next.timeline.tracks[0]?.items.map((entry) => [entry.id, entry.startSeconds])).toEqual([
      ["b-inserted", 1],
      ["a", 2],
      ["b", 5],
    ]);
  });

  it("falls back to the first matching track when the copied track was removed", () => {
    const source = projectWith([track("v1", "video"), track("v2", "video", [item("x", "video_clip", 0, 1)])]);
    const clipboard = requireClipboard(copyItems(source.timeline, ["x"]));
    const target = projectWith([track("v1", "video", [item("a", "video_clip", 0, 1)])]);
    expect(pastePlan(target, clipboard, 2, "paste")).toMatchObject({
      actions: [{ targetTrackId: "v1", items: [{ id: "x-copy", startSeconds: 2 }] }],
    });
  });

  it("is blocked without a clipboard, with too few tracks, or on locked and mismatched tracks", () => {
    const project = baseProject();
    const clipboard = requireClipboard(copyItems(project.timeline, ["t", "a", "m"]));
    expect(pastePlan(project, { entries: [], durationSeconds: 0 }, 0, "paste")).toEqual({
      blocked: "Copy a clip first.",
    });

    const fewerTracks = projectWith([track("text", "overlay"), track("v1", "video")]);
    expect(pastePlan(fewerTracks, clipboard, 0, "paste")).toEqual({
      blocked: "There aren't enough matching tracks to paste these clips.",
    });

    const locked = projectWith([track("text", "overlay"), track("v1", "video", [], true), track("a1", "audio")]);
    expect(pastePlan(locked, clipboard, 0, "paste")).toEqual({ blocked: "Unlock the destination tracks to paste." });

    const mismatched = projectWith([track("text", "overlay"), track("a0", "audio"), track("a1", "audio")]);
    expect(pastePlan(mismatched, clipboard, 0, "paste")).toEqual({
      blocked: "These clips can't go on the destination tracks.",
    });
  });
});
