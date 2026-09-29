import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { trimItemsToPlayhead } from "@/lib/timeline-ops/trim-to-playhead";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, items: TimelineItem[], locked = false): TimelineTrack {
  return { id, name: id, kind: "video", locked, enabled: true, items };
}

function projectWith(tracks: TimelineTrack[]): VideoProject {
  return { ...fixtureProject(), timeline: { durationSeconds: 20, tracks } };
}

const media = clip("media", 2, 4, { properties: { sourceIn: 1, sourceOut: 5 } });
const text = clip("text", 1, 4, { kind: "overlay", source: { type: "text", text: "Hi" } });

describe("trimItemsToPlayhead", () => {
  it("trims start edges to the playhead and moves sourceIn", () => {
    const project = projectWith([track("v1", [media]), track("v2", [text])]);
    expect(trimItemsToPlayhead(project, ["media", "text"], 3, "start")).toEqual({
      actions: [
        {
          type: "trimItems",
          trims: [
            { itemId: "media", startSeconds: 3, durationSeconds: 3, sourceIn: 2, sourceOut: 5 },
            { itemId: "text", startSeconds: 3, durationSeconds: 2 },
          ],
        },
      ],
    });
  });

  it("trims media end edges and resizes clips without a source range", () => {
    const project = projectWith([track("v1", [media]), track("v2", [text])]);
    expect(trimItemsToPlayhead(project, ["media", "text"], 4.5, "end")).toEqual({
      actions: [
        { type: "trimItems", trims: [{ itemId: "media", startSeconds: 2, durationSeconds: 2.5, sourceIn: 1, sourceOut: 3.5 }] },
        { type: "resizeItems", resizes: [{ itemId: "text", durationSeconds: 3.5 }] },
      ],
    });
  });

  it("skips locked clips and clips the playhead is not inside, and explains when nothing trims", () => {
    const project = projectWith([track("v1", [media]), track("v2", [text], true)]);
    expect(trimItemsToPlayhead(project, ["media", "text"], 2, "end")).toEqual({ blocked: "Move the playhead over the selected clip to trim." });
    expect(trimItemsToPlayhead(project, ["text"], 3, "end")).toEqual({ blocked: "Unlock the selected tracks to trim these clips." });
    expect(trimItemsToPlayhead(project, [], 3, "end")).toEqual({ blocked: "Select a clip to trim." });
    expect(trimItemsToPlayhead(project, ["media", "text"], 3, "end")).toMatchObject({ actions: [{ type: "trimItems", trims: [{ itemId: "media" }] }] });
  });
});
