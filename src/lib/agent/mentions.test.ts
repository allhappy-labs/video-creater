import { describe, expect, it } from "vitest";
import {
  activeMentionQuery,
  conversationMentionTargets,
  insertMention,
  parseMentions,
  resolveMentions,
  type MentionTarget,
} from "@/lib/agent/mentions";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";

const targets: MentionTarget[] = [
  { id: "media-1", kind: "media", name: "input.mp4" },
  { id: "media-broll", kind: "media", name: "archive broll.mp4" },
  { id: "item-1", kind: "timelineItem", name: "Opening clip" },
  { id: "item-2", kind: "timelineItem", name: "input.mp4 (0:04)" },
];

describe("parseMentions", () => {
  it("parses bare and quoted names with their ranges", () => {
    const draft = 'Trim @input.mp4 and use @"archive broll.mp4" here';
    expect(parseMentions(draft)).toEqual([
      { name: "input.mp4", start: 5, end: 15, quoted: false },
      { name: "archive broll.mp4", start: 24, end: 44, quoted: true },
    ]);
  });

  it("strips trailing punctuation from bare names and ignores emails, empty and unclosed tokens", () => {
    expect(parseMentions("Cut @input.mp4, then (@clip).").map((mention) => mention.name)).toEqual([
      "input.mp4",
      "clip",
    ]);
    expect(parseMentions('mail oleh@example.com @ @"" @"open quote')).toEqual([]);
  });
});

describe("resolveMentions", () => {
  it("resolves names case-insensitively into focus ids", () => {
    expect(resolveMentions('Use @INPUT.mp4 with @"Opening clip" and @"archive broll.mp4"', targets)).toEqual({
      focus: { mediaIds: ["media-1", "media-broll"], timelineItemIds: ["item-1"] },
      unresolved: [],
    });
  });

  it("reports missing names once each", () => {
    expect(resolveMentions("Use @ghost.mp4 and @Ghost.mp4 and @input.mp4 and @other", targets)).toEqual({
      focus: { mediaIds: ["media-1"], timelineItemIds: [] },
      unresolved: ["ghost.mp4", "other"],
    });
  });

  it("deduplicates repeated mentions and targets sharing a name", () => {
    const shared: MentionTarget[] = [...targets, { id: "media-copy", kind: "media", name: "Input.MP4" }];
    expect(resolveMentions("@input.mp4 then @input.mp4 again @input.mp4", shared).focus).toEqual({
      mediaIds: ["media-1", "media-copy"],
      timelineItemIds: [],
    });
  });

  it("returns an empty focus for a draft without mentions", () => {
    expect(resolveMentions("Tighten the pacing", targets)).toEqual({
      focus: { mediaIds: [], timelineItemIds: [] },
      unresolved: [],
    });
  });
});

describe("activeMentionQuery", () => {
  it("finds the mention being typed before the caret", () => {
    expect(activeMentionQuery("Use @inp", 8)).toEqual({ start: 4, query: "inp" });
    expect(activeMentionQuery('Use @"archive br', 16)).toEqual({ start: 4, query: "archive br" });
    expect(activeMentionQuery("@", 1)).toEqual({ start: 0, query: "" });
    expect(activeMentionQuery("Use @input.mp4 now", 18)).toBeNull();
    expect(activeMentionQuery("oleh@exa", 8)).toBeNull();
  });
});

describe("insertMention", () => {
  it("replaces the typed query and keeps the visible name, which serializes to ids", () => {
    const inserted = insertMention("Tighten @arch", 13, targets[1] as MentionTarget);
    expect(inserted).toEqual({ draft: 'Tighten @"archive broll.mp4" ', caret: 29 });
    expect(inserted.draft).not.toContain("media-broll");
    expect(resolveMentions(inserted.draft, targets)).toEqual({
      focus: { mediaIds: ["media-broll"], timelineItemIds: [] },
      unresolved: [],
    });
  });

  it("inserts at the caret with spacing when no query is active", () => {
    const inserted = insertMention("Trim  please", 5, targets[0] as MentionTarget);
    expect(inserted).toEqual({ draft: "Trim @input.mp4 please", caret: 16 });
    expect(insertMention("Trim", 4, targets[0] as MentionTarget)).toEqual({ draft: "Trim @input.mp4 ", caret: 16 });
  });

  it("quotes names that would not round-trip bare and neutralizes double quotes", () => {
    const punctuated: MentionTarget = { id: "item-9", kind: "timelineItem", name: "Intro!" };
    const quoted: MentionTarget = { id: "item-8", kind: "timelineItem", name: 'The "hero" shot' };
    const first = insertMention("", 0, punctuated);
    expect(first.draft).toBe('@"Intro!" ');
    const second = insertMention(first.draft, first.caret, quoted);
    expect(second.draft).toBe(`@"Intro!" @"The 'hero' shot" `);
    expect(resolveMentions(second.draft, [punctuated, quoted]).focus.timelineItemIds).toEqual(["item-9", "item-8"]);
  });
});

describe("conversationMentionTargets", () => {
  it("lists media and timeline clips by human name without exposing ids as names", () => {
    const project = fixtureProject();
    const found = conversationMentionTargets(project);
    expect(found).toEqual(
      expect.arrayContaining([
        { id: "media-1", kind: "media", name: "input.mp4" },
        { id: "item-1", kind: "timelineItem", name: "Opening clip" },
      ]),
    );
    for (const target of found) expect(target.name).not.toBe(target.id);
  });

  it("disambiguates clip names that collide with media or other clips using the start time", () => {
    const project = fixtureProject();
    const track = fixtureTrack(project, "video");
    const clip = (id: string, startSeconds: number): TimelineItem => ({
      id,
      kind: "video_clip",
      startSeconds,
      durationSeconds: 2,
      source: { type: "media", mediaId: "media-1" },
      label: "input.mp4",
      properties: {},
    });
    track.items = [clip("split-a", 0), clip("split-b", 64), { ...clip("unnamed", 70), label: "  " }];
    const clips = conversationMentionTargets(project).filter((target) => target.kind === "timelineItem");
    expect(clips.filter((target) => target.id.startsWith("split") || target.id === "unnamed")).toEqual([
      { id: "split-a", kind: "timelineItem", name: "input.mp4 (0:00)" },
      { id: "split-b", kind: "timelineItem", name: "input.mp4 (1:04)" },
      { id: "unnamed", kind: "timelineItem", name: "input.mp4 (1:10)" },
    ]);
  });
});
