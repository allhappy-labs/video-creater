import { describe, expect, it } from "vitest";
import { duplicateTimelineItemId, timelineMediaItemId, timelineTransitionId } from "@/lib/timeline-ops/ids";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

function projectWithExistingIds() {
  const project = fixtureProject();
  const video = fixtureItem(project, "video");
  const track = project.timeline.tracks.find((candidate) => candidate.kind === "video");
  track?.items.push(
    ...["timeline-media-1", "timeline-media-1-2", "timeline-media", "item-1-copy", "item-1-copy-2", "item-1-copy-4"].map(
      (id, index) => ({ ...video, id, startSeconds: 20 + index * 5 }),
    ),
  );
  return project;
}

describe("timeline item id characterization", () => {
  it("derives media item ids with collision suffixes", () => {
    const project = projectWithExistingIds();
    const mediaIds = ["media-1", "  Media 1 ", "media-voiceover", "***", "", "--Clip__Name.MOV--"];
    expect(mediaIds.map((mediaId) => timelineMediaItemId(project, mediaId))).toMatchInlineSnapshot(`
      [
        "timeline-media-1-3",
        "timeline-media-1-3",
        "timeline-media-voiceover",
        "timeline-media-2",
        "timeline-media-2",
        "timeline-clip-name-mov",
      ]
    `);
    expect(mediaIds.map((mediaId) => timelineMediaItemId(fixtureProject(), mediaId))).toMatchInlineSnapshot(`
      [
        "timeline-media-1",
        "timeline-media-1",
        "timeline-media-voiceover",
        "timeline-media",
        "timeline-media",
        "timeline-clip-name-mov",
      ]
    `);
  });

  it("derives duplicate item ids with collision suffixes", () => {
    const project = projectWithExistingIds();
    const itemIds = ["item-1", "music-bed", "item-1-copy", ""];
    expect(itemIds.map((itemId) => duplicateTimelineItemId(project, itemId))).toMatchInlineSnapshot(`
      [
        "item-1-copy-3",
        "music-bed-copy",
        "item-1-copy-copy",
        "-copy",
      ]
    `);
    expect(itemIds.map((itemId) => duplicateTimelineItemId(fixtureProject(), itemId))).toMatchInlineSnapshot(`
      [
        "item-1-copy",
        "music-bed-copy",
        "item-1-copy-copy",
        "-copy",
      ]
    `);
  });
});

describe("timeline transition ids", () => {
  it("derives the id from the cut and suffixes collisions across timelines", () => {
    const project = fixtureProject();
    expect(timelineTransitionId(project, "Clip A", "clip_b")).toBe("transition-clip-a-clip-b");
    const taken = { id: "transition-clip-a-clip-b", leftItemId: "x", rightItemId: "y", kind: "crossfade" as const, durationSeconds: 1 };
    project.timeline.tracks[0] = { ...project.timeline.tracks[0]!, transitions: [taken] };
    project.timelines = [{ id: "other", name: "Other", timeline: { durationSeconds: 1, tracks: [{ id: "t", name: "t", kind: "video", locked: false, enabled: true, items: [], transitions: [{ ...taken, id: "transition-clip-a-clip-b-2" }] }] } }];
    expect(timelineTransitionId(project, "Clip A", "clip_b")).toBe("transition-clip-a-clip-b-3");
  });
});
