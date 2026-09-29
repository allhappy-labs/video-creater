import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  buildCaptionItems,
  captionBuildRangeForSourceItem,
  captionRepairActionForItem,
  captionStyleDetails,
  captionStyleProperties,
  captionWordTimings,
  captionWordTokens,
  type CaptionStylePreset,
} from "@/lib/captions/caption-items";
import type { TimelineItem } from "@/lib/timeline";
import { applyProjectActionLocally, type Transcript, type VideoProject } from "@/lib/project";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

const transcript: Transcript = {
  id: "transcript-1",
  mediaId: "media-1",
  repairs: [],
  segments: [],
  words: [
    { text: "Make", startSeconds: 1, endSeconds: 1.2 },
    { text: "this", startSeconds: 1.2, endSeconds: 1.35 },
    { text: "feel", startSeconds: 1.35, endSeconds: 1.55 },
    { text: "cinematic", startSeconds: 1.55, endSeconds: 2 },
    { text: "today", startSeconds: 2, endSeconds: 2.3 },
  ],
};

const stylePresets: CaptionStylePreset[] = ["boldReadableLower", "kineticFocus", "centeredMinimal"];

function captionItemWithTimings(timings: unknown): TimelineItem {
  return {
    id: "caption-timed",
    kind: "caption",
    startSeconds: 2,
    durationSeconds: 1.5,
    source: { type: "text", text: "one two three" },
    label: "Caption timed",
    properties: { captionWordTimings: timings as TimelineItem["properties"][string] },
  };
}

describe("buildCaptionItems", () => {
  it("builds timed caption cues with a shared group and visual contract", () => {
    expect(
      buildCaptionItems({
        transcript,
        range: { startSeconds: 1.1, endSeconds: 2.1 },
        wordsPerCue: 2,
        stylePreset: "kineticFocus",
        groupId: "caption-group-1",
      }),
    ).toEqual([
      expect.objectContaining({
        id: "caption-caption-group-1-1",
        startSeconds: 1.1,
        durationSeconds: 0.25,
        source: { type: "text", text: "Make this" },
        properties: expect.objectContaining({
          transcriptId: "transcript-1",
          wordStartIndex: 0,
          wordEndIndex: 1,
          captionGroupId: "caption-group-1",
          stylePreset: "kineticFocus",
          captionWordTimings: [
            { wordIndex: 0, startSeconds: 0, endSeconds: 0.1 },
            { wordIndex: 1, startSeconds: 0.1, endSeconds: 0.25 },
          ],
          visualTreatment: expect.any(String),
          motion: expect.any(String),
          safeZone: expect.any(String),
          avoid: expect.any(String),
        }),
      }),
      expect.objectContaining({
        id: "caption-caption-group-1-2",
        startSeconds: 1.35,
        durationSeconds: 0.65,
        source: { type: "text", text: "feel cinematic" },
      }),
      expect.objectContaining({
        id: "caption-caption-group-1-3",
        startSeconds: 2,
        durationSeconds: 0.1,
        source: { type: "text", text: "today" },
      }),
    ]);
  });

  it("maps transcript timing through a trimmed playback-rate range", () => {
    const [cue] = buildCaptionItems({
      transcript,
      range: {
        startSeconds: 1,
        endSeconds: 2.3,
        timelineStartSeconds: 8,
        timelineSecondsPerSourceSecond: 0.5,
      },
      wordsPerCue: 2,
      stylePreset: "boldReadableLower",
      groupId: "caption-group-rate",
    });

    expect(cue).toEqual(
      expect.objectContaining({
        startSeconds: 8,
        durationSeconds: 0.175,
        properties: expect.objectContaining({ sourceIn: 1, sourceOut: 1.35 }),
      }),
    );
  });
});

describe("caption item characterization", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-13T00:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("tokenizes caption words on whitespace", () => {
    expect([captionWordTokens("  one  two\tthree\n"), captionWordTokens("")]).toMatchInlineSnapshot(`
      [
        [
          "one",
          "two",
          "three",
        ],
        [],
      ]
    `);
  });

  it("reads stored word timings or falls back to even spacing", () => {
    const validTimings = [
      { wordIndex: 0, startSeconds: 0, endSeconds: 0.4 },
      { wordIndex: 1, startSeconds: 0.4, endSeconds: 0.9 },
      { wordIndex: 2, startSeconds: 0.9, endSeconds: 1.5 },
    ];
    const invalidTimings = [
      { wordIndex: 0, startSeconds: 0.2, endSeconds: 0.1 },
      { wordIndex: 1.5, startSeconds: 0.4, endSeconds: 0.9 },
      { wordIndex: 2, startSeconds: 0.9, endSeconds: 1.6 },
      null,
      "bad",
    ];
    expect({
      withTimings: captionWordTimings(captionItemWithTimings(validTimings), 3),
      withoutTimings: captionWordTimings(captionItemWithTimings(undefined), 3),
      fewerWordsThanTimings: captionWordTimings(captionItemWithTimings(validTimings), 2),
      moreWordsThanTimings: captionWordTimings(captionItemWithTimings(validTimings), 4),
      invalidTimings: captionWordTimings(captionItemWithTimings(invalidTimings), 3),
      nullItem: captionWordTimings(null, 2),
      zeroWords: captionWordTimings(captionItemWithTimings(validTimings), 0),
    }).toMatchInlineSnapshot(`
      {
        "fewerWordsThanTimings": [
          {
            "endSeconds": 0.4,
            "startSeconds": 0,
            "wordIndex": 0,
          },
          {
            "endSeconds": 0.9,
            "startSeconds": 0.4,
            "wordIndex": 1,
          },
        ],
        "invalidTimings": [
          {
            "endSeconds": 0.5,
            "startSeconds": 0,
            "wordIndex": 0,
          },
          {
            "endSeconds": 1,
            "startSeconds": 0.5,
            "wordIndex": 1,
          },
          {
            "endSeconds": 1.5,
            "startSeconds": 1,
            "wordIndex": 2,
          },
        ],
        "moreWordsThanTimings": [
          {
            "endSeconds": 0.4,
            "startSeconds": 0,
            "wordIndex": 0,
          },
          {
            "endSeconds": 0.9,
            "startSeconds": 0.4,
            "wordIndex": 1,
          },
          {
            "endSeconds": 1.5,
            "startSeconds": 0.9,
            "wordIndex": 2,
          },
          {
            "endSeconds": 1.5,
            "startSeconds": 1.125,
            "wordIndex": 3,
          },
        ],
        "nullItem": [
          {
            "endSeconds": 0,
            "startSeconds": 0,
            "wordIndex": 0,
          },
          {
            "endSeconds": 0,
            "startSeconds": 0,
            "wordIndex": 1,
          },
        ],
        "withTimings": [
          {
            "endSeconds": 0.4,
            "startSeconds": 0,
            "wordIndex": 0,
          },
          {
            "endSeconds": 0.9,
            "startSeconds": 0.4,
            "wordIndex": 1,
          },
          {
            "endSeconds": 1.5,
            "startSeconds": 0.9,
            "wordIndex": 2,
          },
        ],
        "withoutTimings": [
          {
            "endSeconds": 0.5,
            "startSeconds": 0,
            "wordIndex": 0,
          },
          {
            "endSeconds": 1,
            "startSeconds": 0.5,
            "wordIndex": 1,
          },
          {
            "endSeconds": 1.5,
            "startSeconds": 1,
            "wordIndex": 2,
          },
        ],
        "zeroWords": [],
      }
    `);
  });

  it("describes style details and properties for every preset", () => {
    expect(
      stylePresets.map((preset) => ({
        preset,
        details: captionStyleDetails[preset],
        properties: captionStyleProperties(preset),
      })),
    ).toMatchInlineSnapshot(`
      [
        {
          "details": {
            "avoid": "full-width opaque black slabs, faces, hands, and main action",
            "motion": "quick upward pop-in, short hold, and soft fade out",
            "safeZone": "keep essential text inside 10% margins and above the lower safe area",
            "visualTreatment": "bold phone-readable lower-third caption with a shaped translucent backing and accent stroke",
          },
          "preset": "boldReadableLower",
          "properties": {
            "avoid": "full-width opaque black slabs, faces, hands, and main action",
            "motion": "quick upward pop-in, short hold, and soft fade out",
            "motionPresetId": "snap-pop-v1",
            "safeZone": "keep essential text inside 10% margins and above the lower safe area",
            "stylePreset": "boldReadableLower",
            "visualTreatment": "bold phone-readable lower-third caption with a shaped translucent backing and accent stroke",
          },
        },
        {
          "details": {
            "avoid": "static text-only cards, default-font template looks, and long unmoving holds",
            "motion": "word-aware scale and tracking accent on the stressed phrase, then a short ease out",
            "safeZone": "keep essential text inside 10% margins and away from faces or product details",
            "visualTreatment": "kinetic emphasized caption with selective word scale, restrained highlight color, and transparent backing",
          },
          "preset": "kineticFocus",
          "properties": {
            "avoid": "static text-only cards, default-font template looks, and long unmoving holds",
            "motion": "word-aware scale and tracking accent on the stressed phrase, then a short ease out",
            "motionPresetId": "pulse-emphasis-v2",
            "safeZone": "keep essential text inside 10% margins and away from faces or product details",
            "stylePreset": "kineticFocus",
            "visualTreatment": "kinetic emphasized caption with selective word scale, restrained highlight color, and transparent backing",
          },
        },
        {
          "details": {
            "avoid": "opaque boxes, edge-to-edge text, and covering faces, hands, or product details",
            "motion": "brief fade and 2% scale settle on entry, then a soft fade out",
            "safeZone": "keep essential text inside 12% margins and clear of the primary action",
            "visualTreatment": "compact centered caption with soft shadow, subtle material backing, and generous breathing room",
          },
          "preset": "centeredMinimal",
          "properties": {
            "avoid": "opaque boxes, edge-to-edge text, and covering faces, hands, or product details",
            "motion": "brief fade and 2% scale settle on entry, then a soft fade out",
            "motionPresetId": "soft-depth-card-v2",
            "safeZone": "keep essential text inside 12% margins and clear of the primary action",
            "stylePreset": "centeredMinimal",
            "visualTreatment": "compact centered caption with soft shadow, subtle material backing, and generous breathing room",
          },
        },
      ]
    `);
  });

  it("derives a caption build range from a trimmed source item", () => {
    const item = fixtureItem(fixtureProject(), "video");
    const trimmedAtRate: TimelineItem = {
      ...item,
      startSeconds: 3,
      durationSeconds: 2,
      properties: { ...item.properties, sourceIn: 1, sourceOut: 4, playbackRate: 1.5 },
    };
    const withoutSourceOut: TimelineItem = {
      ...item,
      startSeconds: 3,
      durationSeconds: 2,
      properties: { sourceIn: 1.25 },
    };
    const invertedSourceRange: TimelineItem = {
      ...item,
      startSeconds: 3,
      durationSeconds: 2,
      properties: { sourceIn: 4, sourceOut: 2 },
    };
    expect({
      trimmedAtRate: captionBuildRangeForSourceItem(trimmedAtRate),
      withoutSourceOut: captionBuildRangeForSourceItem(withoutSourceOut),
      invertedSourceRange: captionBuildRangeForSourceItem(invertedSourceRange),
    }).toMatchInlineSnapshot(`
      {
        "invertedSourceRange": {
          "endSeconds": 6,
          "startSeconds": 4,
          "timelineSecondsPerSourceSecond": 1,
          "timelineStartSeconds": 3,
        },
        "trimmedAtRate": {
          "endSeconds": 4,
          "startSeconds": 1,
          "timelineSecondsPerSourceSecond": 0.6666666666666666,
          "timelineStartSeconds": 3,
        },
        "withoutSourceOut": {
          "endSeconds": 3.25,
          "startSeconds": 1.25,
          "timelineSecondsPerSourceSecond": 1,
          "timelineStartSeconds": 3,
        },
      }
    `);
  });

  it("scales the implicit caption source range by clip playback speed", () => {
    const item = fixtureItem(fixtureProject(), "video");
    const fastWithoutSourceOut: TimelineItem = {
      ...item,
      startSeconds: 3,
      durationSeconds: 2,
      properties: { sourceIn: 1.25, speed: 2 },
    };
    const fastWithSourceOut: TimelineItem = {
      ...item,
      startSeconds: 3,
      durationSeconds: 2,
      properties: { sourceIn: 1.25, sourceOut: 5.25, speed: 2 },
    };

    const expected = {
      startSeconds: 1.25,
      endSeconds: 5.25,
      timelineStartSeconds: 3,
      timelineSecondsPerSourceSecond: 0.5,
    };
    expect(captionBuildRangeForSourceItem(fastWithoutSourceOut)).toEqual(expected);
    expect(captionBuildRangeForSourceItem(fastWithSourceOut)).toEqual(expected);
  });

  it("builds caption repair actions for transcript-linked caption items", () => {
    const project = fixtureProject();
    const caption = fixtureItem(project, "caption");
    const video = fixtureItem(project, "video");
    expect({
      unchangedText: captionRepairActionForItem(caption, "Original caption text", project),
      changedText: captionRepairActionForItem(caption, "Corrected caption text", project),
      nonCaptionItem: captionRepairActionForItem(video, "Corrected caption text", project),
      negativeWordIndex: captionRepairActionForItem(
        { ...caption, properties: { ...caption.properties, wordIndex: -1 } },
        "Corrected caption text",
        project,
      ),
    }).toMatchInlineSnapshot(`
      {
        "changedText": null,
        "negativeWordIndex": null,
        "nonCaptionItem": null,
        "unchangedText": null,
      }
    `);
  });
});

describe("captionRepairActionForItem", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-13T00:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** The sample project with Caption 1 retimed to transcript word 0 ("Original", 0.65–1.05 s). */
  function singleWordProject(): VideoProject {
    const project = fixtureProject();
    const caption = fixtureItem(project, "caption");
    caption.startSeconds = 0.65;
    caption.durationSeconds = 0.4;
    caption.source = { type: "text", text: "Original" };
    return project;
  }

  it("does not repair a multi-word cue that maps to one transcript word", () => {
    const project = fixtureProject();
    expect(captionRepairActionForItem(fixtureItem(project, "caption"), "Restored voice", project)).toBeNull();
    expect(captionRepairActionForItem(fixtureItem(project, "caption"), "Restored", project)).toBeNull();
  });

  it("repairs a single-word cue whose timing matches the word, using the word's timing", () => {
    const project = singleWordProject();
    const caption = fixtureItem(project, "caption");
    caption.durationSeconds = 0.4004;
    const action = captionRepairActionForItem(caption, "Restored", project);
    expect(action).toEqual({
      type: "applyCaptionRepair",
      repair: {
        captionItemId: "caption-1",
        transcriptId: "transcript-media-1",
        wordIndex: 0,
        text: "Restored",
        startSeconds: 0.65,
        endSeconds: 1.05,
        repairId: "caption-repair-caption-1-mtz1s000",
        createdAt: "2026-09-13T00:00:00.000Z",
      },
    });

    const next = applyProjectActionLocally(project, action!);
    expect(fixtureItem(next, "caption")).toMatchObject({
      startSeconds: 0.65,
      durationSeconds: 0.4,
      source: { type: "text", text: "Restored" },
      properties: { textEdited: true, wordIndex: 0 },
    });
    expect(next.transcripts[0]?.words[0]).toMatchObject({ text: "Restored", startSeconds: 0.65, endSeconds: 1.05 });
  });

  it("does not repair when the new text is more than one word", () => {
    const project = singleWordProject();
    expect(captionRepairActionForItem(fixtureItem(project, "caption"), "Restored voice", project)).toBeNull();
  });

  it("does not repair when the cue timing is off the word by more than 1 ms", () => {
    const project = singleWordProject();
    const caption = fixtureItem(project, "caption");
    expect(captionRepairActionForItem({ ...caption, startSeconds: 0.66, durationSeconds: 0.39 }, "Restored", project)).toBeNull();
    expect(captionRepairActionForItem({ ...caption, durationSeconds: 0.45 }, "Restored", project)).toBeNull();
  });

  it("does not repair a cue built from a range of words, or a missing transcript or word", () => {
    const project = singleWordProject();
    const caption = fixtureItem(project, "caption");
    const withProperties = (properties: Record<string, unknown>): TimelineItem => ({
      ...caption,
      properties: { ...caption.properties, ...properties },
    });
    expect(captionRepairActionForItem(withProperties({ wordStartIndex: 0, wordEndIndex: 1 }), "Restored", project)).toBeNull();
    expect(captionRepairActionForItem(withProperties({ transcriptId: "missing" }), "Restored", project)).toBeNull();
    expect(captionRepairActionForItem(withProperties({ wordIndex: 9 }), "Restored", project)).toBeNull();
    expect(captionRepairActionForItem(caption, "Restored", { transcripts: [] })).toBeNull();
  });
});
