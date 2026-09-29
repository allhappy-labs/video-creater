import { describe, expect, it } from "vitest";
import {
  automationPointPosition,
  automationValueBounds,
  overviewWaveformPeaks,
  sourceBoundaryRange,
  sourceRangeDurationMismatch,
  suppliedWaveformPeaks,
  timelineAutomationPoints,
  waveformPeaks,
} from "@/lib/timeline-ops/automation";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

function automationItems(): TimelineItem[] {
  const project = fixtureProject();
  const video = fixtureItem(project, "video");
  const audio = fixtureItem(project, "audio");
  return [
    { ...audio, id: "audio-no-fades", durationSeconds: 4, properties: {} },
    {
      ...audio,
      id: "audio-fades-volume",
      durationSeconds: 4,
      properties: {
        fadeInSeconds: 1,
        fadeOutSeconds: 20,
        keyframes: {
          volumeDb: [
            { atSeconds: 3, value: -6, easing: "easeOut" },
            { atSeconds: 0, value: -60 },
            { atSeconds: 1.5, value: 30, easing: 7 },
          ],
          opacity: [{ atSeconds: 1, value: 0.5 }],
        },
      },
    },
    {
      ...video,
      id: "video-unsorted-opacity",
      durationSeconds: 5,
      properties: {
        keyframes: {
          opacity: [
            { atSeconds: 4, value: 1 },
            { atSeconds: 0, value: 0, easing: "hold" },
            { atSeconds: 2, value: 1.5 },
            { atSeconds: 5, value: -0.2 },
            { atSeconds: 5.01, value: 1 },
            { atSeconds: -0.1, value: 1 },
            { atSeconds: "1", value: 1 },
            { atSeconds: 1, value: Number.NaN },
            null,
            [1, 2],
          ],
        },
      },
    },
    {
      ...video,
      id: "video-single-opacity",
      durationSeconds: 5,
      properties: { keyframes: { opacity: [{ atSeconds: 2.5, value: 0.25 }] } },
    },
    { ...video, id: "video-empty-opacity", durationSeconds: 5, properties: { keyframes: { opacity: [] } } },
    { ...video, id: "video-keyframes-array", durationSeconds: 5, properties: { keyframes: [] } },
    { ...video, id: "video-keyframes-not-array", durationSeconds: 5, properties: { keyframes: { opacity: "1" } } },
  ];
}

describe("timeline automation characterization", () => {
  it("reads automation points", () => {
    expect(
      automationItems().map((item) => [
        item.id,
        timelineAutomationPoints(item, "opacity"),
        timelineAutomationPoints(item, "volumeDb"),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "audio-no-fades",
          [],
          [],
        ],
        [
          "audio-fades-volume",
          [
            {
              "atSeconds": 1,
              "easing": undefined,
              "value": 0.5,
            },
          ],
          [
            {
              "atSeconds": 0,
              "easing": undefined,
              "value": -60,
            },
            {
              "atSeconds": 1.5,
              "easing": undefined,
              "value": 30,
            },
            {
              "atSeconds": 3,
              "easing": "easeOut",
              "value": -6,
            },
          ],
        ],
        [
          "video-unsorted-opacity",
          [
            {
              "atSeconds": 0,
              "easing": "hold",
              "value": 0,
            },
            {
              "atSeconds": 2,
              "easing": undefined,
              "value": 1.5,
            },
            {
              "atSeconds": 4,
              "easing": undefined,
              "value": 1,
            },
            {
              "atSeconds": 5,
              "easing": undefined,
              "value": -0.2,
            },
          ],
          [],
        ],
        [
          "video-single-opacity",
          [
            {
              "atSeconds": 2.5,
              "easing": undefined,
              "value": 0.25,
            },
          ],
          [],
        ],
        [
          "video-empty-opacity",
          [],
          [],
        ],
        [
          "video-keyframes-array",
          [],
          [],
        ],
        [
          "video-keyframes-not-array",
          [],
          [],
        ],
      ]
    `);
  });

  it("positions automation points", () => {
    expect([automationValueBounds("opacity"), automationValueBounds("volumeDb")]).toMatchInlineSnapshot(`
      [
        {
          "maximum": 1,
          "minimum": 0,
          "step": 0.05,
        },
        {
          "maximum": 24,
          "minimum": -60,
          "step": 0.5,
        },
      ]
    `);
    const [, audio, video] = automationItems();
    expect(
      [
        { atSeconds: 0, value: 0 },
        { atSeconds: 2.5, value: 0.5 },
        { atSeconds: 5, value: 1 },
        { atSeconds: 1.234567, value: 1.5 },
        { atSeconds: 7, value: -1 },
      ].map((point) => automationPointPosition(video!, "opacity", point)),
    ).toMatchInlineSnapshot(`
      [
        {
          "left": 0,
          "top": 100,
        },
        {
          "left": 50,
          "top": 50,
        },
        {
          "left": 100,
          "top": 0,
        },
        {
          "left": 24.69,
          "top": 0,
        },
        {
          "left": 140,
          "top": 100,
        },
      ]
    `);
    expect(
      [
        { atSeconds: 0, value: -60 },
        { atSeconds: 1, value: 0 },
        { atSeconds: 4, value: 24 },
        { atSeconds: 2, value: 100 },
      ].map((point) => automationPointPosition(audio!, "volumeDb", point)),
    ).toMatchInlineSnapshot(`
      [
        {
          "left": 0,
          "top": 100,
        },
        {
          "left": 25,
          "top": 28.57,
        },
        {
          "left": 100,
          "top": 0,
        },
        {
          "left": 50,
          "top": 0,
        },
      ]
    `);
    expect(
      automationPointPosition({ ...video!, durationSeconds: 0 }, "opacity", { atSeconds: 1, value: 0.5 }),
    ).toMatchInlineSnapshot(`
      {
        "left": Infinity,
        "top": 50,
      }
    `);
  });

  it("normalizes waveform peaks", () => {
    const project = fixtureProject();
    const audio = fixtureItem(project, "audio");
    const items: TimelineItem[] = [
      audio,
      { ...audio, id: "supplied", properties: { waveformPeaks: [0.5, -0.25, 1.5, Number.NaN, "0.4", 0, 1] } },
      { ...audio, id: "supplied-invalid", properties: { waveformPeaks: [Number.NaN, "0.4", null] } },
      { ...audio, id: "supplied-empty", properties: { waveformPeaks: [] } },
      { ...audio, id: "supplied-not-array", properties: { waveformPeaks: "0.5" } },
      { ...audio, id: "fallback-short", durationSeconds: 0.5, properties: {} },
    ];
    expect(items.map((item) => [item.id, suppliedWaveformPeaks(item)])).toMatchInlineSnapshot(`
      [
        [
          "music-bed",
          [
            0.22,
            0.38,
            0.3,
            0.62,
            0.48,
            0.76,
            0.42,
            0.58,
            0.8,
            0.44,
            0.54,
            0.72,
            0.5,
            0.34,
            0.64,
            0.86,
            0.48,
            0.7,
            0.56,
            0.42,
            0.68,
            0.58,
            0.36,
            0.24,
          ],
        ],
        [
          "supplied",
          [
            0.5,
            0,
            1,
            0,
            1,
          ],
        ],
        [
          "supplied-invalid",
          null,
        ],
        [
          "supplied-empty",
          null,
        ],
        [
          "supplied-not-array",
          null,
        ],
        [
          "fallback-short",
          null,
        ],
      ]
    `);
    expect(items.map((item) => [item.id, waveformPeaks(item)])).toMatchInlineSnapshot(`
      [
        [
          "music-bed",
          [
            0.22,
            0.38,
            0.3,
            0.62,
            0.48,
            0.76,
            0.42,
            0.58,
            0.8,
            0.44,
            0.54,
            0.72,
            0.5,
            0.34,
            0.64,
            0.86,
            0.48,
            0.7,
            0.56,
            0.42,
            0.68,
            0.58,
            0.36,
            0.24,
          ],
        ],
        [
          "supplied",
          [
            0.5,
            0,
            1,
            0,
            1,
          ],
        ],
        [
          "supplied-invalid",
          [
            0.65,
            0.47,
            0.62,
            0.41,
            0.26,
            0.3,
            0.58,
            0.45,
            0.72,
            0.26,
            0.51,
            0.58,
            0.23,
            0.87,
            0.25,
            0.8,
            0.48,
            0.48,
            0.83,
            0.23,
            0.93,
            0.3,
            0.66,
            0.61,
          ],
        ],
        [
          "supplied-empty",
          [
            0.49,
            0.44,
            0.72,
            0.34,
            0.59,
            0.48,
            0.29,
            0.83,
            0.27,
            0.86,
            0.38,
            0.57,
            0.75,
            0.27,
            0.94,
            0.25,
            0.76,
            0.53,
            0.37,
            0.81,
            0.32,
            0.78,
            0.22,
            0.43,
          ],
        ],
        [
          "supplied-not-array",
          [
            0.37,
            0.62,
            0.71,
            0.27,
            0.9,
            0.25,
            0.72,
            0.41,
            0.32,
            0.69,
            0.41,
            0.69,
            0.37,
            0.35,
            0.34,
            0.51,
            0.5,
            0.67,
            0.34,
            0.49,
            0.51,
            0.23,
            0.83,
            0.28,
          ],
        ],
        [
          "fallback-short",
          [
            0.58,
            0.39,
            0.9,
            0.22,
            0.88,
            0.38,
            0.56,
            0.71,
            0.23,
            0.85,
            0.3,
            0.63,
            0.38,
            0.23,
            0.63,
            0.5,
            0.59,
            0.43,
            0.22,
            0.28,
            0.62,
            0.43,
            0.74,
            0.23,
          ],
        ],
      ]
    `);
    expect(
      items.map((item) => [item.id, overviewWaveformPeaks(item.properties.waveformPeaks)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "music-bed",
          [
            0.22,
            0.38,
            0.3,
            0.62,
            0.48,
            0.76,
            0.42,
            0.58,
            0.8,
            0.44,
            0.54,
            0.72,
            0.5,
            0.34,
            0.64,
            0.86,
            0.48,
            0.7,
            0.56,
            0.42,
            0.68,
            0.58,
            0.36,
            0.24,
          ],
        ],
        [
          "supplied",
          [
            0.5,
            -0.25,
            1.5,
            0,
            1,
          ],
        ],
        [
          "supplied-invalid",
          [],
        ],
        [
          "supplied-empty",
          [],
        ],
        [
          "supplied-not-array",
          [],
        ],
        [
          "fallback-short",
          [],
        ],
      ]
    `);
  });

  it("compares source spans to clip duration times playback speed", () => {
    const video = fixtureItem(fixtureProject(), "video");
    const fast = { ...video, durationSeconds: 2, properties: { sourceIn: 1, sourceOut: 5, speed: 2 } };
    const fastIgnoringSpeed = { ...fast, properties: { sourceIn: 1, sourceOut: 3, speed: 2 } };

    expect(sourceRangeDurationMismatch(fast)).toBeNull();
    expect(sourceRangeDurationMismatch(fastIgnoringSpeed)).toEqual({
      sourceSpan: 2,
      clipDuration: 2,
    });
  });

  it("reads source boundaries and duration mismatches", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const items: TimelineItem[] = [
      { ...video, id: "matching", durationSeconds: 4, properties: { sourceIn: 1, sourceOut: 5 } },
      { ...video, id: "within-tolerance", durationSeconds: 4, properties: { sourceIn: 1, sourceOut: 5.009 } },
      { ...video, id: "outside-tolerance", durationSeconds: 4, properties: { sourceIn: 1, sourceOut: 5.011 } },
      { ...video, id: "slow-motion", durationSeconds: 8, properties: { sourceIn: 0, sourceOut: 4, speed: 0.5 } },
      { ...video, id: "rounding", durationSeconds: 0.3, properties: { sourceIn: 0.1, sourceOut: 0.4 } },
      { ...video, id: "inverted", durationSeconds: 4, properties: { sourceIn: 5, sourceOut: 1 } },
      { ...video, id: "empty-range", durationSeconds: 4, properties: { sourceIn: 2, sourceOut: 2 } },
      { ...video, id: "missing-out", durationSeconds: 4, properties: { sourceIn: 2 } },
      { ...video, id: "string-range", durationSeconds: 4, properties: { sourceIn: "1", sourceOut: "5" } },
      {
        ...video,
        id: "generated",
        durationSeconds: 2,
        source: { type: "generated", artifactId: "artifact-1" },
        properties: { sourceIn: 0, sourceOut: 4 },
      },
      {
        ...video,
        id: "text",
        kind: "caption",
        durationSeconds: 2,
        source: { type: "text", text: "Hello" },
        properties: { sourceIn: 0, sourceOut: 4 },
      },
    ];
    expect(
      items.map((item) => [item.id, sourceBoundaryRange(item), sourceRangeDurationMismatch(item)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "matching",
          {
            "sourceIn": 1,
            "sourceOut": 5,
          },
          null,
        ],
        [
          "within-tolerance",
          {
            "sourceIn": 1,
            "sourceOut": 5.009,
          },
          null,
        ],
        [
          "outside-tolerance",
          {
            "sourceIn": 1,
            "sourceOut": 5.011,
          },
          {
            "clipDuration": 4,
            "sourceSpan": 4.011,
          },
        ],
        [
          "slow-motion",
          {
            "sourceIn": 0,
            "sourceOut": 4,
          },
          null,
        ],
        [
          "rounding",
          {
            "sourceIn": 0.1,
            "sourceOut": 0.4,
          },
          null,
        ],
        [
          "inverted",
          null,
          null,
        ],
        [
          "empty-range",
          null,
          null,
        ],
        [
          "missing-out",
          null,
          null,
        ],
        [
          "string-range",
          null,
          null,
        ],
        [
          "generated",
          {
            "sourceIn": 0,
            "sourceOut": 4,
          },
          {
            "clipDuration": 2,
            "sourceSpan": 4,
          },
        ],
        [
          "text",
          null,
          null,
        ],
      ]
    `);
  });

  it("positions keyframes against explicit value bounds", () => {
    const [, , video] = automationItems();
    expect(automationPointPosition(video!, { minimum: -360, maximum: 360 }, { atSeconds: 1.25, value: 180 })).toEqual({ left: 25, top: 25 });
  });
});
