import { describe, expect, it } from "vitest";
import { deadAirRangesForItem } from "@/lib/timeline-ops/dead-air";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

function projectWithSilence(ranges: NonNullable<VideoProject["mediaSilenceRanges"]>) {
  const project = fixtureProject();
  project.mediaSilenceRanges = ranges;
  return project;
}

function mediaItem(project: VideoProject, overrides: Partial<TimelineItem>): TimelineItem {
  return {
    ...fixtureItem(project, "video"),
    source: { type: "media", mediaId: "media-1" },
    ...overrides,
  };
}

describe("deadAirRangesForItem", () => {
  it("maps source silence through a trimmed item at speed 1.5 and clips to the item bounds", () => {
    const project = projectWithSilence([
      // Fully inside the item's source range [4, 13].
      { mediaId: "media-1", sourceIn: 7, sourceOut: 8.5, confidence: 0.9 },
      // Starts before sourceIn: clipped to the item's left edge.
      { mediaId: "media-1", sourceIn: 2, sourceOut: 5.5, confidence: 0.8 },
      // Ends after the source range: clipped to the item's right edge.
      { mediaId: "media-1", sourceIn: 11.5, sourceOut: 20, confidence: 0.7 },
      // Entirely outside the source range.
      { mediaId: "media-1", sourceIn: 14, sourceOut: 16, confidence: 1 },
    ]);
    // 6 s on the timeline at speed 1.5 consumes 9 s of source: [4, 13].
    const item = mediaItem(project, {
      startSeconds: 10,
      durationSeconds: 6,
      properties: { sourceIn: 4, speed: 1.5 },
    });

    expect(deadAirRangesForItem(project, item)).toEqual([
      { startSeconds: 10, endSeconds: 11, startRatio: 0, endRatio: 0.1667 },
      { startSeconds: 12, endSeconds: 13, startRatio: 0.3333, endRatio: 0.5 },
      { startSeconds: 15, endSeconds: 16, startRatio: 0.8333, endRatio: 1 },
    ]);
  });

  it("uses an explicit sourceOut as the right source bound", () => {
    const project = projectWithSilence([
      { mediaId: "media-1", sourceIn: 3, sourceOut: 9, confidence: 1 },
    ]);
    const item = mediaItem(project, {
      startSeconds: 0,
      durationSeconds: 4,
      properties: { sourceIn: 2, sourceOut: 6 },
    });

    expect(deadAirRangesForItem(project, item)).toEqual([
      { startSeconds: 1, endSeconds: 4, startRatio: 0.25, endRatio: 1 },
    ]);
  });

  it("draws silence where a reversed clip plays it", () => {
    const project = projectWithSilence([
      { mediaId: "media-1", sourceIn: 1, sourceOut: 2, confidence: 1 },
      // Starts before sourceIn, so it reaches the reversed clip's right edge.
      { mediaId: "media-1", sourceIn: -1, sourceOut: 0.5, confidence: 1 },
    ]);
    const reversed = mediaItem(project, { startSeconds: 0, durationSeconds: 10, properties: { sourceIn: 0, sourceOut: 10, reverse: true } });
    const fast = mediaItem(project, { startSeconds: 20, durationSeconds: 5, properties: { sourceIn: 0, sourceOut: 10, speed: 2, reverse: true } });

    expect(deadAirRangesForItem(project, reversed)).toEqual([
      { startSeconds: 8, endSeconds: 9, startRatio: 0.8, endRatio: 0.9 },
      { startSeconds: 9.5, endSeconds: 10, startRatio: 0.95, endRatio: 1 },
    ]);
    expect(deadAirRangesForItem(project, fast)).toEqual([
      { startSeconds: 24, endSeconds: 24.5, startRatio: 0.8, endRatio: 0.9 },
      { startSeconds: 24.75, endSeconds: 25, startRatio: 0.95, endRatio: 1 },
    ]);
  });

  it("merges overlapping silence ranges and ignores invalid ones", () => {
    const project = projectWithSilence([
      { mediaId: "media-1", sourceIn: 2, sourceOut: 3, confidence: 1 },
      { mediaId: "media-1", sourceIn: 1, sourceOut: 2.5, confidence: 1 },
      { mediaId: "media-1", sourceIn: 5, sourceOut: 5, confidence: 1 },
      { mediaId: "media-1", sourceIn: 6, sourceOut: 7, confidence: Number.NaN },
      { mediaId: "  ", sourceIn: 6, sourceOut: 7, confidence: 1 },
      { mediaId: "media-other", sourceIn: 6, sourceOut: 7, confidence: 1 },
    ]);
    const item = mediaItem(project, { startSeconds: 0, durationSeconds: 8, properties: {} });

    expect(deadAirRangesForItem(project, item)).toEqual([
      { startSeconds: 1, endSeconds: 3, startRatio: 0.125, endRatio: 0.375 },
    ]);
  });

  it("returns no ranges for non-media sources, missing silence data, or invalid timing", () => {
    const project = projectWithSilence([
      { mediaId: "media-1", sourceIn: 0, sourceOut: 5, confidence: 1 },
    ]);
    const base = mediaItem(project, { startSeconds: 0, durationSeconds: 4, properties: {} });

    expect(deadAirRangesForItem(project, { ...base, source: { type: "timeline", timelineId: "t" } })).toEqual([]);
    expect(deadAirRangesForItem(project, { ...base, durationSeconds: 0 })).toEqual([]);
    expect(deadAirRangesForItem(project, { ...base, properties: { speed: 0 } })).toEqual([]);
    expect(deadAirRangesForItem(fixtureProject(), base)).toEqual([]);
  });
});
