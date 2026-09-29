import { describe, expect, it } from "vitest";
import {
  nestedSequenceItems,
  planCreateTimeline,
  planDeleteTimeline,
  planRenameTimeline,
  planSetActiveTimeline,
  projectTimelineEntries,
} from "@/lib/timeline-ops/multi-timeline";
import type { VideoProject } from "@/lib/project";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";

function multiTimelineProject(): VideoProject {
  const project = fixtureProject();
  const empty = { durationSeconds: 0, tracks: [] };
  project.timelines = [
    { id: "main", name: "Main edit", timeline: project.timeline },
    { id: "timeline-2", name: "B-roll", timeline: empty },
    { id: "timeline-3", name: "Intro", timeline: empty },
  ];
  project.activeTimelineId = "main";
  return project;
}

describe("projectTimelineEntries", () => {
  it("falls back to a single implicit timeline for legacy projects", () => {
    const project = fixtureProject();
    expect(projectTimelineEntries(project)).toEqual([
      { id: "main", name: "Timeline 1", timeline: project.timeline },
    ]);
  });
});

describe("planCreateTimeline", () => {
  it("creates an empty timeline named after the next count", () => {
    expect(planCreateTimeline(fixtureProject(), { duplicate: false })).toEqual({
      actions: [
        { type: "createTimeline", timelineId: "timeline-2", name: "Timeline 2", duplicateActive: false },
      ],
    });
  });

  it("suffixes the id with -1, -2 on collisions", () => {
    const project = fixtureProject();
    const empty = () => ({ durationSeconds: 0, tracks: [] });
    project.activeTimelineId = "main";
    project.timelines = [
      { id: "main", name: "Main", timeline: project.timeline },
      { id: "timeline-3", name: "Taken", timeline: empty() },
    ];
    // 2 entries: base id timeline-3 is taken.
    expect(planCreateTimeline(project, { duplicate: false })).toMatchObject({
      actions: [{ timelineId: "timeline-3-1", name: "Timeline 3" }],
    });
    project.timelines = [
      { id: "main", name: "Main", timeline: project.timeline },
      { id: "timeline-4", name: "Taken", timeline: empty() },
      { id: "timeline-4-1", name: "Taken", timeline: empty() },
    ];
    // 3 entries: timeline-4 and timeline-4-1 are taken.
    expect(planCreateTimeline(project, { duplicate: false })).toMatchObject({
      actions: [{ timelineId: "timeline-4-2", name: "Timeline 4" }],
    });
  });

  it("duplicates the active timeline by default as 'Copy of <name>'", () => {
    expect(planCreateTimeline(multiTimelineProject(), { duplicate: true })).toEqual({
      actions: [
        {
          type: "createTimeline",
          timelineId: "timeline-4",
          name: "Copy of Main edit",
          duplicateActive: true,
          sourceTimelineId: "main",
        },
      ],
    });
  });

  it("duplicates a named source timeline, and blocks a missing one", () => {
    const project = multiTimelineProject();
    expect(planCreateTimeline(project, { duplicate: true, sourceTimelineId: "timeline-3" })).toMatchObject({
      actions: [{ name: "Copy of Intro", sourceTimelineId: "timeline-3" }],
    });
    expect(planCreateTimeline(project, { duplicate: true, sourceTimelineId: "gone" })).toEqual({
      blocked: "That timeline no longer exists.",
    });
  });
});

describe("planRenameTimeline", () => {
  it("emits renameTimeline with a trimmed name", () => {
    expect(planRenameTimeline(multiTimelineProject(), "timeline-2", "  Cutaways ")).toEqual({
      actions: [{ type: "renameTimeline", timelineId: "timeline-2", name: "Cutaways" }],
    });
  });

  it("blocks blank names and missing timelines", () => {
    const project = multiTimelineProject();
    expect(planRenameTimeline(project, "timeline-2", "   ")).toEqual({ blocked: "Enter a timeline name." });
    expect(planRenameTimeline(project, "gone", "Name")).toEqual({ blocked: "That timeline no longer exists." });
  });
});

describe("planDeleteTimeline", () => {
  it("emits deleteTimeline", () => {
    expect(planDeleteTimeline(multiTimelineProject(), "timeline-2")).toEqual({
      actions: [{ type: "deleteTimeline", timelineId: "timeline-2" }],
    });
  });

  it("blocks deleting the only timeline", () => {
    const project = fixtureProject();
    expect(planDeleteTimeline(project, "main")).toEqual({
      blocked: "A project needs at least one timeline.",
    });
  });

  it("blocks missing timelines and timelines used as nested sequences", () => {
    const project = multiTimelineProject();
    fixtureTrack(project, "video").items.push({
      ...fixtureItem(project, "video"),
      id: "nested-intro",
      source: { type: "timeline", timelineId: "timeline-3" },
    });
    expect(planDeleteTimeline(project, "gone")).toEqual({ blocked: "That timeline no longer exists." });
    expect(planDeleteTimeline(project, "timeline-3")).toEqual({
      blocked: "Intro is used as a nested sequence. Decompose or remove it first.",
    });
  });
});

describe("planSetActiveTimeline", () => {
  it("emits setActiveTimeline for another timeline", () => {
    expect(planSetActiveTimeline(multiTimelineProject(), "timeline-3")).toEqual({
      actions: [{ type: "setActiveTimeline", timelineId: "timeline-3" }],
    });
  });

  it("is a no-op for the active timeline and blocked for a missing one", () => {
    const project = multiTimelineProject();
    expect(planSetActiveTimeline(project, "main")).toEqual({ actions: [] });
    expect(planSetActiveTimeline(project, "gone")).toEqual({ blocked: "That timeline no longer exists." });
  });
});

describe("nestedSequenceItems", () => {
  it("lists active timeline items whose source is another timeline", () => {
    const project = multiTimelineProject();
    const video = fixtureItem(project, "video");
    const nestedIntro = { ...video, id: "nested-intro", source: { type: "timeline" as const, timelineId: "timeline-3" } };
    const nestedMissing = { ...video, id: "nested-missing", source: { type: "timeline" as const, timelineId: "gone" } };
    const selfReference = { ...video, id: "self", source: { type: "timeline" as const, timelineId: "main" } };
    fixtureTrack(project, "video").items.push(nestedIntro, selfReference);
    fixtureTrack(project, "audio").items.push(nestedMissing);

    expect(nestedSequenceItems(project)).toEqual([
      { trackId: fixtureTrack(project, "video").id, item: nestedIntro, timelineId: "timeline-3", timelineName: "Intro" },
      { trackId: fixtureTrack(project, "audio").id, item: nestedMissing, timelineId: "gone", timelineName: null },
    ]);
  });

  it("returns an empty list when nothing is nested", () => {
    expect(nestedSequenceItems(fixtureProject())).toEqual([]);
  });
});
