import { describe, expect, it } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { transcriptWordFixActions } from "./transcript-word-fix";

const now = new Date("2026-09-14T12:00:00.000Z");

function captionEdits(actions: readonly ProjectAction[]) {
  return actions.flatMap((action) => (action.type === "editCaptionText" ? [{ itemId: action.itemId, text: action.text }] : []));
}

function builtCue(overrides: Partial<TimelineItem> & { properties?: TimelineItem["properties"] }): TimelineItem {
  return {
    id: "cue",
    kind: "caption",
    startSeconds: 0.65,
    durationSeconds: 2.7,
    source: { type: "text", text: "Original caption Second split" },
    label: "Caption 1",
    ...overrides,
    properties: { transcriptId: "transcript-media-1", wordStartIndex: 0, wordEndIndex: 3, ...overrides.properties },
  };
}

function withCaptions(items: TimelineItem[], locked = false): VideoProject {
  const project = fixtureProject();
  project.timeline.tracks = project.timeline.tracks.map((track) => (track.kind === "caption" ? { ...track, locked, items } : track));
  return project;
}

describe("transcriptWordFixActions", () => {
  it("edits the transcript word with a legacy repair id and shared timestamp", () => {
    const result = transcriptWordFixActions(withCaptions([]), { transcriptId: "transcript-media-1", wordIndex: 1, text: "  captions " }, now);
    expect(result).toEqual({
      actions: [
        {
          type: "editTranscriptWords",
          edits: [
            {
              transcriptId: "transcript-media-1",
              wordIndex: 1,
              text: "captions",
              repairId: `word-repair-transcript-media-1-1-${now.getTime().toString(36)}`,
              createdAt: "2026-09-14T12:00:00.000Z",
            },
          ],
        },
      ],
    });
  });

  it("replaces the word in built cues by its position in the cue", () => {
    const project = withCaptions([builtCue({ source: { type: "text", text: "Original caption Second split" } })]);
    const result = transcriptWordFixActions(project, { transcriptId: "transcript-media-1", wordIndex: 3, text: "splits" }, now);
    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.actions[0]?.type).toBe("editTranscriptWords");
    expect(captionEdits(result.actions)).toEqual([{ itemId: "cue", text: "Original caption Second splits" }]);
  });

  it("keeps punctuation around a matched word when the cue text no longer lines up with its words", () => {
    // The sample cues are single-word repairs whose text has an extra word; the old word is matched instead.
    const result = transcriptWordFixActions(fixtureProject(), { transcriptId: "transcript-media-1", wordIndex: 0, text: "Restored" }, now);
    if ("blocked" in result) throw new Error(result.blocked);
    expect(captionEdits(result.actions)).toEqual([{ itemId: "caption-1", text: "Restored caption text" }]);

    const punctuated = withCaptions([builtCue({ source: { type: "text", text: "“Original,” he said" }, properties: { wordStartIndex: 0, wordEndIndex: 1 } })]);
    const quoted = transcriptWordFixActions(punctuated, { transcriptId: "transcript-media-1", wordIndex: 0, text: "Restored" }, now);
    if ("blocked" in quoted) throw new Error(quoted.blocked);
    expect(captionEdits(quoted.actions)).toEqual([{ itemId: "cue", text: "“Restored,” he said" }]);
  });

  it("leaves cues that don't contain the word or belong to another transcript", () => {
    const project = withCaptions([
      builtCue({ id: "later", properties: { wordStartIndex: 2, wordEndIndex: 3 }, source: { type: "text", text: "Second split" } }),
      builtCue({ id: "other", properties: { transcriptId: "transcript-other" } }),
      builtCue({ id: "rewritten", properties: { wordStartIndex: 0, wordEndIndex: 1 }, source: { type: "text", text: "Something else entirely" } }),
    ]);
    const result = transcriptWordFixActions(project, { transcriptId: "transcript-media-1", wordIndex: 0, text: "Restored" }, now);
    if ("blocked" in result) throw new Error(result.blocked);
    expect(captionEdits(result.actions)).toEqual([]);
  });

  it("is a no-op for unchanged text and blocks empty text, missing words and locked cues", () => {
    const project = fixtureProject();
    expect(transcriptWordFixActions(project, { transcriptId: "transcript-media-1", wordIndex: 0, text: "Original" }, now)).toEqual({ actions: [] });
    expect(transcriptWordFixActions(project, { transcriptId: "transcript-media-1", wordIndex: 0, text: "   " }, now)).toEqual({ blocked: "A word can't be empty." });
    expect(transcriptWordFixActions(project, { transcriptId: "transcript-media-1", wordIndex: 9, text: "x" }, now)).toEqual({
      blocked: "That word is no longer in the transcript.",
    });
    expect(transcriptWordFixActions(project, { transcriptId: "gone", wordIndex: 0, text: "x" }, now)).toEqual({
      blocked: "That transcript is no longer in the project.",
    });
    const locked = withCaptions([builtCue({})], true);
    expect(transcriptWordFixActions(locked, { transcriptId: "transcript-media-1", wordIndex: 0, text: "Restored" }, now)).toEqual({
      blocked: "Unlock the caption track to fix this word.",
    });
  });
});
