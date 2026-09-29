import { describe, expect, it } from "vitest";
import { buildCaptionItems } from "@/lib/captions/caption-items";
import type { Transcript, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { captionEmphasisAction, captionRegroup, captionWordsPerCue } from "./caption-edits";

const transcript: Transcript = {
  id: "transcript-1",
  mediaId: "media-1",
  repairs: [],
  segments: [],
  words: ["The", "first", "recorded", "voice", "in", "history"].map((text, index) => ({
    text,
    startSeconds: index,
    endSeconds: index + 0.8,
  })),
};

function cues(wordsPerCue: number, groupId = "group-a"): TimelineItem[] {
  return buildCaptionItems({
    transcript,
    range: { startSeconds: 0, endSeconds: 6, timelineStartSeconds: 10, timelineSecondsPerSourceSecond: 1 },
    wordsPerCue,
    stylePreset: "kineticFocus",
    groupId,
  });
}

function project(items: TimelineItem[], locked = false): VideoProject {
  return {
    ...fixtureProject(),
    transcripts: [transcript],
    timeline: {
      durationSeconds: 20,
      tracks: [{ id: "captions", name: "Captions", kind: "caption", locked, enabled: true, items }],
    },
  };
}

describe("captionWordsPerCue", () => {
  it("is the longest cue in the group by transcript word span", () => {
    expect(captionWordsPerCue(project(cues(4)), "caption-group-a-2")).toBe(4);
  });
});

describe("captionRegroup", () => {
  it("removes the group's cues and adds the rebuilt cues in one batch with the same group id", () => {
    const old = cues(3).map((cue) => ({ ...cue, properties: { ...cue.properties, captionPlacement: "upper", fontSize: 64 } }));
    const result = captionRegroup(project(old), "caption-group-a-2", 2);
    if ("blocked" in result) throw new Error(result.blocked);

    const rebuilt = cues(2).map((cue) => ({ ...cue, properties: { ...cue.properties, captionPlacement: "upper", fontSize: 64 } }));
    expect(result.fromCount).toBe(2);
    expect(result.toCount).toBe(3);
    expect(result.actions).toEqual([
      { type: "removeItems", itemIds: ["caption-group-a-1", "caption-group-a-2"] },
      { type: "addItems", targetTrackId: "captions", items: rebuilt },
    ]);
    // The selected cue started at the 4th word, which the 2nd rebuilt cue covers.
    expect(result.selectItemId).toBe("caption-group-a-2");
  });

  it("blocks without a transcript, a group id, an unlocked track or a valid word count", () => {
    const [first] = cues(3);
    if (!first) throw new Error("missing cue");
    const ungrouped = { ...first, properties: { ...first.properties, captionGroupId: undefined } };
    expect(captionRegroup(project([ungrouped]), first.id, 2)).toEqual({ blocked: "This caption isn't part of a caption group." });
    expect(captionRegroup({ ...project(cues(3)), transcripts: [] }, first.id, 2)).toEqual({
      blocked: "This caption group has no transcript to regroup from.",
    });
    expect(captionRegroup(project(cues(3), true), first.id, 2)).toEqual({ blocked: "Unlock the caption track to regroup its cues." });
    expect(captionRegroup(project(cues(3)), first.id, 13)).toEqual({ blocked: "Max words per line must be a whole number from 1 to 12." });
  });
});

describe("captionEmphasisAction", () => {
  const cue: TimelineItem = {
    id: "cue",
    kind: "caption",
    startSeconds: 0,
    durationSeconds: 3,
    source: { type: "text", text: "The first recorded" },
    label: "Caption 1",
    properties: { emphasizedWordIndices: [2, 7] },
  };

  it("adds a word in ascending order and drops indices outside the cue", () => {
    expect(captionEmphasisAction(cue, 0)).toEqual({
      actions: [{ type: "updateItemProperties", updates: [{ itemId: "cue", set: { emphasizedWordIndices: [0, 2] }, remove: [] }] }],
    });
  });

  it("removes the property when the last word is cleared", () => {
    expect(captionEmphasisAction(cue, 2)).toEqual({
      actions: [{ type: "updateItemProperties", updates: [{ itemId: "cue", set: {}, remove: ["emphasizedWordIndices"] }] }],
    });
  });

  it("rebuilds the word animations in the same update when a word preset is applied", () => {
    const animated = { ...cue, properties: { emphasizedWordIndices: [], captionWordAnimationPreset: "groupPulse", captionWordStaggerSeconds: 0 } };
    const result = captionEmphasisAction(animated, 1);
    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.actions).toEqual([
      {
        type: "updateItemProperties",
        updates: [
          {
            itemId: "cue",
            set: {
              emphasizedWordIndices: [1],
              captionWordAnimations: [
                expect.objectContaining({ wordIndex: 1, emphasisScale: 1.08, easing: "outQuad" }),
              ],
            },
            remove: [],
          },
        ],
      },
    ]);
  });
});
