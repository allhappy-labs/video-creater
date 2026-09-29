import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { saveRangeTarget, saveRangeUnavailableReason } from "@/lib/timeline-ops/save-range";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function clip(id: string, startSeconds: number, durationSeconds: number): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {} };
}

function track(id: string, items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind: "video", locked: false, enabled: true, items };
}

/** "a" 1–3 s and "b" 5.5–7 s on v1, "c" 2–9.25 s on v2; the timeline is 12 s long. */
function projectWith(): VideoProject {
  return { ...fixtureProject(), timeline: { durationSeconds: 12, tracks: [track("v1", [clip("a", 1, 2), clip("b", 5.5, 1.5)]), track("v2", [clip("c", 2, 7.25)])] } };
}

const noMarks = { rangeIn: null, rangeOut: null };

describe("saveRangeTarget", () => {
  it("uses a selected clip's timeline span", () => {
    expect(saveRangeTarget(projectWith(), { selectedItemIds: ["b"], ...noMarks })).toEqual({ range: { startSeconds: 5.5, endSeconds: 7 }, source: "selection" });
  });

  it("spans every selected clip across tracks", () => {
    expect(saveRangeTarget(projectWith(), { selectedItemIds: ["a", "c"], ...noMarks })).toEqual({ range: { startSeconds: 1, endSeconds: 9.25 }, source: "selection" });
  });

  it("prefers the selection over I/O marks", () => {
    expect(saveRangeTarget(projectWith(), { selectedItemIds: ["a"], rangeIn: 4, rangeOut: 8 })).toMatchObject({ range: { startSeconds: 1, endSeconds: 3 }, source: "selection" });
  });

  it("uses the I/O range when nothing on this timeline is selected", () => {
    expect(saveRangeTarget(projectWith(), { selectedItemIds: ["gone"], rangeIn: 4, rangeOut: 8 })).toEqual({ range: { startSeconds: 4, endSeconds: 8 }, source: "marks" });
  });

  it("ends the I/O range at the end of the timeline", () => {
    expect(saveRangeTarget(projectWith(), { selectedItemIds: [], rangeIn: 10, rangeOut: 30 })).toMatchObject({ range: { startSeconds: 10, endSeconds: 12 } });
  });

  it.each([
    ["no selection or marks", { selectedItemIds: [], ...noMarks }],
    ["only an in point", { selectedItemIds: [], rangeIn: 2, rangeOut: null }],
    ["only an out point", { selectedItemIds: [], rangeIn: null, rangeOut: 2 }],
    ["an out point before the in point", { selectedItemIds: [], rangeIn: 6, rangeOut: 4 }],
    ["marks past the end of the timeline", { selectedItemIds: [], rangeIn: 14, rangeOut: 18 }],
  ])("is unavailable with %s", (_label, context) => {
    expect(saveRangeTarget(projectWith(), context)).toEqual({ blocked: "Select a clip or mark in and out points" });
  });

  it("exports the menu reason", () => {
    expect(saveRangeUnavailableReason).toBe("Select a clip or mark in and out points");
  });
});
