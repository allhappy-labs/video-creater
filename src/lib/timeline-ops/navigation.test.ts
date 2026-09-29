import { describe, expect, it } from "vitest";
import {
  buildTimelineTrackGeometry,
  clamp,
  clampTrackDisplayHeight,
  clampZoomPercent,
  finiteOr,
  formatInteractionTimecode,
  formatTimestamp,
  interactionWindow,
  isEditableKeyboardTarget,
  nextTimelineEditPoint,
  normalizedDuration,
  normalizeWindow,
  previousTimelineEditPoint,
  renderedBadgeSeparation,
  resolveTimelineOverviewViewState,
  snapTimelineSeconds,
  timelineBadgeLayout,
  timelineEditPointSeconds,
  timelineGapAtSeconds,
  timelineInteractionErrorFeedback,
  timelineItemDensity,
  trackDisplayHeight,
  trackEnabled,
  trackKindAccentLabel,
  trackLaneLabels,
  trackLanePrefix,
  windowsMatch,
} from "@/lib/timeline-ops/navigation";
import type { Timeline, TimelineItem, TimelineTrack, TrackKind } from "@/lib/timeline";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";

const trackKinds: TrackKind[] = ["video", "hyperframe_scene", "overlay", "caption", "audio"];
const playheads = [0, 1.999, 2, 4.5, 8, 20];

function navigationTimeline(): Timeline {
  const project = fixtureProject();
  const video = fixtureItem(project, "video");
  const clip = (id: string, startSeconds: number, durationSeconds: number): TimelineItem => ({
    ...video,
    id,
    startSeconds,
    durationSeconds,
  });
  const videoTrack = fixtureTrack(project, "video");
  const audioTrack = fixtureTrack(project, "audio");
  videoTrack.items = [clip("video-a", 0, 2), clip("video-b", 2, 2), clip("video-c", 5, 3)];
  audioTrack.items = [{ ...clip("audio-a", 1, 2), kind: "audio_clip" }];
  return { durationSeconds: 10, tracks: [videoTrack, audioTrack] };
}

describe("timeline navigation characterization", () => {
  it("finds only bounded empty track gaps", () => {
    const track = {
      id: "video",
      name: "Video",
      kind: "video" as const,
      locked: false,
      items: [
        { id: "a", kind: "video_clip" as const, startSeconds: 0, durationSeconds: 1, source: { type: "media" as const, mediaId: "a" }, label: "A", properties: {} },
        { id: "b", kind: "video_clip" as const, startSeconds: 3, durationSeconds: 1, source: { type: "media" as const, mediaId: "b" }, label: "B", properties: {} },
      ],
    };
    expect(timelineGapAtSeconds(track, 2)).toEqual({
      trackId: "video",
      startSeconds: 1,
      endSeconds: 3,
    });
    expect(timelineGapAtSeconds(track, 0.5)).toBeNull();
    expect(timelineGapAtSeconds(track, 5)).toBeNull();
  });

  it("probes gaps on the two-track timeline", () => {
    const timeline = navigationTimeline();
    expect(
      timeline.tracks.map((track) => [
        track.id,
        [0, 1, 2, 4, 4.5, 5, 8, 20].map((seconds) => [seconds, timelineGapAtSeconds(track, seconds)]),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "track-video",
          [
            [
              0,
              null,
            ],
            [
              1,
              null,
            ],
            [
              2,
              null,
            ],
            [
              4,
              {
                "endSeconds": 5,
                "startSeconds": 4,
                "trackId": "track-video",
              },
            ],
            [
              4.5,
              {
                "endSeconds": 5,
                "startSeconds": 4,
                "trackId": "track-video",
              },
            ],
            [
              5,
              null,
            ],
            [
              8,
              null,
            ],
            [
              20,
              null,
            ],
          ],
        ],
        [
          "track-audio",
          [
            [
              0,
              {
                "endSeconds": 1,
                "startSeconds": 0,
                "trackId": "track-audio",
              },
            ],
            [
              1,
              null,
            ],
            [
              2,
              null,
            ],
            [
              4,
              null,
            ],
            [
              4.5,
              null,
            ],
            [
              5,
              null,
            ],
            [
              8,
              null,
            ],
            [
              20,
              null,
            ],
          ],
        ],
      ]
    `);
    const emptyTrack: TimelineTrack = { ...timeline.tracks[0]!, items: [] };
    expect(timelineGapAtSeconds(emptyTrack, 1)).toMatchInlineSnapshot(`null`);
  });

  it("formats timestamps identically across timeline copies", () => {
    const seconds = [0, 0.5, 1.999, 59.999, 60, 61.25, 3599.5, 3600, 7322.125, -1, -61, Number.NaN];
    const expected = seconds.map((value) => formatTimestamp(value));
    expect(expected).toMatchInlineSnapshot(`
      [
        "00:00",
        "00:00",
        "00:01",
        "00:59",
        "01:00",
        "01:01",
        "59:59",
        "60:00",
        "122:02",
        "-1:-1",
        "-2:-1",
        "NaN:NaN",
      ]
    `);
  });

  it("walks edit points from every playhead", () => {
    const timeline = navigationTimeline();
    const editPoints = timelineEditPointSeconds(timeline);
    expect(editPoints).toMatchInlineSnapshot(`
      [
        0,
        1,
        2,
        3,
        4,
        5,
        8,
        10,
      ]
    `);
    expect(
      playheads.map((playhead) => [
        playhead,
        previousTimelineEditPoint(editPoints, playhead),
        nextTimelineEditPoint(editPoints, playhead),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          null,
          1,
        ],
        [
          1.999,
          1,
          3,
        ],
        [
          2,
          1,
          3,
        ],
        [
          4.5,
          4,
          5,
        ],
        [
          8,
          5,
          10,
        ],
        [
          20,
          10,
          null,
        ],
      ]
    `);
    expect(
      [1.9995, 2.0005, 2.0011].map((playhead) => [
        playhead,
        previousTimelineEditPoint(editPoints, playhead),
        nextTimelineEditPoint(editPoints, playhead),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          1.9995,
          1,
          3,
        ],
        [
          2.0005,
          1,
          3,
        ],
        [
          2.0011,
          2,
          3,
        ],
      ]
    `);
    expect(timelineEditPointSeconds({ ...timeline, durationSeconds: 6 })).toMatchInlineSnapshot(`
      [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
      ]
    `);
    expect(timelineEditPointSeconds({ durationSeconds: 0, tracks: [] })).toMatchInlineSnapshot(`
      [
        0,
      ]
    `);
    expect([previousTimelineEditPoint([], 1), nextTimelineEditPoint([], 1)]).toMatchInlineSnapshot(`
      [
        null,
        null,
      ]
    `);
  });

  it("clamps zoom and snaps seconds", () => {
    expect(
      [0, 1, 49, 50, 99, 100, 200, 201, 1000, Number.NaN, Number.POSITIVE_INFINITY, -1].map((zoom) => [
        zoom,
        clampZoomPercent(zoom),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          50,
        ],
        [
          1,
          50,
        ],
        [
          49,
          50,
        ],
        [
          50,
          50,
        ],
        [
          99,
          99,
        ],
        [
          100,
          100,
        ],
        [
          200,
          200,
        ],
        [
          201,
          200,
        ],
        [
          1000,
          200,
        ],
        [
          NaN,
          100,
        ],
        [
          Infinity,
          100,
        ],
        [
          -1,
          50,
        ],
      ]
    `);
    expect(
      [0, 0.05, 0.1, 0.124, 0.125, 0.126, 0.5, 1.999, 2.05, 4.5, 4.88, -0.13, Number.NaN, Number.POSITIVE_INFINITY].map(
        (seconds) => [seconds, snapTimelineSeconds(seconds)],
      ),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          0,
        ],
        [
          0.05,
          0,
        ],
        [
          0.1,
          0,
        ],
        [
          0.124,
          0,
        ],
        [
          0.125,
          0.25,
        ],
        [
          0.126,
          0.25,
        ],
        [
          0.5,
          0.5,
        ],
        [
          1.999,
          2,
        ],
        [
          2.05,
          2,
        ],
        [
          4.5,
          4.5,
        ],
        [
          4.88,
          5,
        ],
        [
          -0.13,
          -0.25,
        ],
        [
          NaN,
          0,
        ],
        [
          Infinity,
          0,
        ],
      ]
    `);
  });

  it("resolves overview view state", () => {
    const current = { startSeconds: 2, endSeconds: 12 };
    const cases: Array<[string, Parameters<typeof resolveTimelineOverviewViewState>[0]]> = [
      ["unchanged", { currentWindow: current, requestedWindow: current, durationSeconds: 30, viewportWidth: 800 }],
      ["inside pan", { currentWindow: current, requestedWindow: { startSeconds: 4, endSeconds: 14 }, durationSeconds: 30, viewportWidth: 800 }],
      ["start only", { currentWindow: current, requestedWindow: { startSeconds: 7, endSeconds: 12 }, durationSeconds: 30, viewportWidth: 800 }],
      ["end only", { currentWindow: current, requestedWindow: { startSeconds: 2, endSeconds: 22 }, durationSeconds: 30, viewportWidth: 800 }],
      ["overflowing end", { currentWindow: current, requestedWindow: { startSeconds: 25, endSeconds: 35 }, durationSeconds: 30, viewportWidth: 800 }],
      ["negative start", { currentWindow: current, requestedWindow: { startSeconds: -6, endSeconds: 4 }, durationSeconds: 30, viewportWidth: 800 }],
      ["wider than duration", { currentWindow: current, requestedWindow: { startSeconds: -10, endSeconds: 60 }, durationSeconds: 30, viewportWidth: 800 }],
      ["tiny window", { currentWindow: current, requestedWindow: { startSeconds: 5, endSeconds: 5.01 }, durationSeconds: 30, viewportWidth: 800 }],
      ["non-finite request", { currentWindow: current, requestedWindow: { startSeconds: Number.NaN, endSeconds: Number.POSITIVE_INFINITY }, durationSeconds: 30, viewportWidth: 800 }],
      ["non-finite current", { currentWindow: { startSeconds: Number.NaN, endSeconds: Number.NaN }, requestedWindow: { startSeconds: 1, endSeconds: 3 }, durationSeconds: 30, viewportWidth: 800 }],
      ["non-finite duration and width", { currentWindow: current, requestedWindow: { startSeconds: 4, endSeconds: 14 }, durationSeconds: Number.NaN, viewportWidth: Number.NaN }],
      ["zero width", { currentWindow: current, requestedWindow: { startSeconds: 4, endSeconds: 14 }, durationSeconds: 30, viewportWidth: 0 }],
    ];
    expect(cases.map(([name, input]) => [name, resolveTimelineOverviewViewState(input)])).toMatchInlineSnapshot(`
      [
        [
          "unchanged",
          {
            "scrollLeft": 160,
            "window": {
              "endSeconds": 12,
              "startSeconds": 2,
            },
            "zoomPercent": 100,
          },
        ],
        [
          "inside pan",
          {
            "scrollLeft": 320,
            "window": {
              "endSeconds": 14,
              "startSeconds": 4,
            },
            "zoomPercent": 100,
          },
        ],
        [
          "start only",
          {
            "scrollLeft": 1120,
            "window": {
              "endSeconds": 12,
              "startSeconds": 7,
            },
            "zoomPercent": 200,
          },
        ],
        [
          "end only",
          {
            "scrollLeft": 80,
            "window": {
              "endSeconds": 22,
              "startSeconds": 2,
            },
            "zoomPercent": 50,
          },
        ],
        [
          "overflowing end",
          {
            "scrollLeft": 1600,
            "window": {
              "endSeconds": 30,
              "startSeconds": 20,
            },
            "zoomPercent": 100,
          },
        ],
        [
          "negative start",
          {
            "scrollLeft": 0,
            "window": {
              "endSeconds": 10,
              "startSeconds": 0,
            },
            "zoomPercent": 100,
          },
        ],
        [
          "wider than duration",
          {
            "scrollLeft": 400,
            "window": {
              "endSeconds": 30,
              "startSeconds": 10,
            },
            "zoomPercent": 50,
          },
        ],
        [
          "tiny window",
          {
            "scrollLeft": 400.8,
            "window": {
              "endSeconds": 7.505,
              "startSeconds": 2.505,
            },
            "zoomPercent": 200,
          },
        ],
        [
          "non-finite request",
          {
            "scrollLeft": 160,
            "window": {
              "endSeconds": 12,
              "startSeconds": 2,
            },
            "zoomPercent": 100,
          },
        ],
        [
          "non-finite current",
          {
            "scrollLeft": 0,
            "window": {
              "endSeconds": 5,
              "startSeconds": 0,
            },
            "zoomPercent": 200,
          },
        ],
        [
          "non-finite duration and width",
          {
            "scrollLeft": 0,
            "window": {
              "endSeconds": 0,
              "startSeconds": 0,
            },
            "zoomPercent": 50,
          },
        ],
        [
          "zero width",
          {
            "scrollLeft": 360,
            "window": {
              "endSeconds": 9,
              "startSeconds": 9,
            },
            "zoomPercent": 50,
          },
        ],
      ]
    `);
  });

  it("labels tracks", () => {
    const project = fixtureProject();
    const tracks: TimelineTrack[] = [
      ...project.timeline.tracks,
      { ...fixtureTrack(project, "video"), id: "track-video-2", enabled: false },
      { ...fixtureTrack(project, "audio"), id: "track-audio-2", enabled: true },
    ];
    expect(
      [undefined, ...tracks].map((track) => [track?.id ?? null, trackEnabled(track)]),
    ).toMatchInlineSnapshot(`
      [
        [
          null,
          true,
        ],
        [
          "track-video",
          true,
        ],
        [
          "track-scenes",
          true,
        ],
        [
          "track-overlays",
          true,
        ],
        [
          "track-captions",
          true,
        ],
        [
          "track-audio",
          true,
        ],
        [
          "track-video-2",
          false,
        ],
        [
          "track-audio-2",
          true,
        ],
      ]
    `);
    expect(trackKinds.map((kind) => [kind, trackLanePrefix(kind), trackKindAccentLabel(kind)])).toMatchInlineSnapshot(`
      [
        [
          "video",
          "V",
          "video",
        ],
        [
          "hyperframe_scene",
          "H",
          "HyperFrames",
        ],
        [
          "overlay",
          "O",
          "overlay",
        ],
        [
          "caption",
          "C",
          "caption",
        ],
        [
          "audio",
          "A",
          "audio",
        ],
      ]
    `);
    expect(Array.from(trackLaneLabels(tracks))).toMatchInlineSnapshot(`
      [
        [
          "track-video",
          "V1",
        ],
        [
          "track-scenes",
          "H1",
        ],
        [
          "track-overlays",
          "O1",
        ],
        [
          "track-captions",
          "C1",
        ],
        [
          "track-audio",
          "A1",
        ],
        [
          "track-video-2",
          "V2",
        ],
        [
          "track-audio-2",
          "A2",
        ],
      ]
    `);
    expect(Array.from(trackLaneLabels([]))).toMatchInlineSnapshot(`[]`);
  });

  it("maps native interaction errors to feedback", () => {
    expect(
      [
        "timeline items overlap on track track-video: item-1 overlaps video-after",
        "  Timeline Items Overlap On Track track-video : item-1   overlaps   video-after  ",
        "timeline items overlap on track track-video: item-1",
        "Project is locked",
        "",
        "   ",
      ].map((message) => timelineInteractionErrorFeedback(message)),
    ).toMatchInlineSnapshot(`
      [
        {
          "displayMessage": "Overlaps video-after",
          "itemId": "item-1",
          "nativeMessage": "timeline items overlap on track track-video: item-1 overlaps video-after",
          "targetTrackId": "track-video",
        },
        {
          "displayMessage": "Overlaps video-after",
          "itemId": "item-1",
          "nativeMessage": "  Timeline Items Overlap On Track track-video : item-1   overlaps   video-after  ",
          "targetTrackId": "track-video",
        },
        {
          "displayMessage": "timeline items overlap on track track-video: item-1",
          "itemId": null,
          "nativeMessage": "timeline items overlap on track track-video: item-1",
          "targetTrackId": null,
        },
        {
          "displayMessage": "Project is locked",
          "itemId": null,
          "nativeMessage": "Project is locked",
          "targetTrackId": null,
        },
        null,
        null,
      ]
    `);
  });

  it("detects editable keyboard targets identically", () => {
    const contentEditable = document.createElement("div");
    contentEditable.contentEditable = "true";
    const targets: Array<[string, EventTarget | null]> = [
      ["input", document.createElement("input")],
      ["textarea", document.createElement("textarea")],
      ["select", document.createElement("select")],
      ["contenteditable div", contentEditable],
      ["button", document.createElement("button")],
      ["plain div", document.createElement("div")],
      ["document", document],
      ["window", window],
      ["null", null],
    ];
    const expected = targets.map(([name, target]) => [name, isEditableKeyboardTarget(target)]);
    expect(expected).toMatchInlineSnapshot(`
      [
        [
          "input",
          true,
        ],
        [
          "textarea",
          true,
        ],
        [
          "select",
          true,
        ],
        [
          "contenteditable div",
          false,
        ],
        [
          "button",
          false,
        ],
        [
          "plain div",
          false,
        ],
        [
          "document",
          false,
        ],
        [
          "window",
          false,
        ],
        [
          "null",
          false,
        ],
      ]
    `);
  });

  it("builds track display geometry", () => {
    const project = fixtureProject();
    const withHeight = (kind: TrackKind, displayHeight: unknown) =>
      ({ ...fixtureTrack(project, kind), id: `${kind}-${String(displayHeight)}`, displayHeight }) as TimelineTrack;
    const tracks: TimelineTrack[] = [
      ...project.timeline.tracks,
      withHeight("video", 10),
      withHeight("caption", 100),
      withHeight("audio", 500),
      withHeight("overlay", Number.NaN),
      withHeight("overlay", "90"),
    ];
    expect(tracks.map((track) => [track.id, trackDisplayHeight(track)])).toMatchInlineSnapshot(`
      [
        [
          "track-video",
          64,
        ],
        [
          "track-scenes",
          48,
        ],
        [
          "track-overlays",
          48,
        ],
        [
          "track-captions",
          48,
        ],
        [
          "track-audio",
          64,
        ],
        [
          "video-10",
          44,
        ],
        [
          "caption-100",
          100,
        ],
        [
          "audio-500",
          200,
        ],
        [
          "overlay-NaN",
          48,
        ],
        [
          "overlay-90",
          48,
        ],
      ]
    `);
    expect(
      [0, 43, 44, 100, 200, 201, -5, Number.NaN, Number.POSITIVE_INFINITY].map((value) => [
        value,
        clampTrackDisplayHeight(value),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          44,
        ],
        [
          43,
          44,
        ],
        [
          44,
          44,
        ],
        [
          100,
          100,
        ],
        [
          200,
          200,
        ],
        [
          201,
          200,
        ],
        [
          -5,
          44,
        ],
        [
          NaN,
          NaN,
        ],
        [
          Infinity,
          200,
        ],
      ]
    `);
    const geometry = buildTimelineTrackGeometry(tracks);
    expect({
      entries: geometry.entries,
      byIdKeys: Array.from(geometry.byId.keys()),
      totalHeight: geometry.totalHeight,
    }).toMatchInlineSnapshot(`
      {
        "byIdKeys": [
          "track-video",
          "track-scenes",
          "track-overlays",
          "track-captions",
          "track-audio",
          "video-10",
          "caption-100",
          "audio-500",
          "overlay-NaN",
          "overlay-90",
        ],
        "entries": [
          {
            "bottom": 64,
            "height": 64,
            "index": 0,
            "top": 0,
            "trackId": "track-video",
          },
          {
            "bottom": 112,
            "height": 48,
            "index": 1,
            "top": 64,
            "trackId": "track-scenes",
          },
          {
            "bottom": 160,
            "height": 48,
            "index": 2,
            "top": 112,
            "trackId": "track-overlays",
          },
          {
            "bottom": 208,
            "height": 48,
            "index": 3,
            "top": 160,
            "trackId": "track-captions",
          },
          {
            "bottom": 272,
            "height": 64,
            "index": 4,
            "top": 208,
            "trackId": "track-audio",
          },
          {
            "bottom": 316,
            "height": 44,
            "index": 5,
            "top": 272,
            "trackId": "video-10",
          },
          {
            "bottom": 416,
            "height": 100,
            "index": 6,
            "top": 316,
            "trackId": "caption-100",
          },
          {
            "bottom": 616,
            "height": 200,
            "index": 7,
            "top": 416,
            "trackId": "audio-500",
          },
          {
            "bottom": 664,
            "height": 48,
            "index": 8,
            "top": 616,
            "trackId": "overlay-NaN",
          },
          {
            "bottom": 712,
            "height": 48,
            "index": 9,
            "top": 664,
            "trackId": "overlay-90",
          },
        ],
        "totalHeight": 712,
      }
    `);
    expect(
      [-10, 0, 63.999, 64, 111.999, 112, geometry.totalHeight - 0.0005, geometry.totalHeight, 100000, Number.NaN].map(
        (y) => [y, geometry.trackIndexAtY(y)],
      ),
    ).toMatchInlineSnapshot(`
      [
        [
          -10,
          0,
        ],
        [
          0,
          0,
        ],
        [
          63.999,
          0,
        ],
        [
          64,
          1,
        ],
        [
          111.999,
          1,
        ],
        [
          112,
          2,
        ],
        [
          711.9995,
          9,
        ],
        [
          712,
          9,
        ],
        [
          100000,
          9,
        ],
        [
          NaN,
          9,
        ],
      ]
    `);
    const empty = buildTimelineTrackGeometry([]);
    expect([empty.entries, empty.totalHeight, empty.byId.size, empty.trackIndexAtY(0)]).toMatchInlineSnapshot(`
      [
        [],
        0,
        0,
        -1,
      ]
    `);
  });

  it("normalizes overview windows", () => {
    expect(
      [[1, 2], [Number.NaN, 2], [Number.POSITIVE_INFINITY, 2], [-3, 2]].map(([value, fallback]) =>
        finiteOr(value!, fallback!),
      ),
    ).toMatchInlineSnapshot(`
      [
        1,
        2,
        2,
        -3,
      ]
    `);
    const clampInputs: Array<[number, number, number]> = [
      [5, 0, 10],
      [-1, 0, 10],
      [11, 0, 10],
      [5, 10, 0],
      [Number.NaN, 0, 10],
    ];
    const expectedClamp = clampInputs.map(([value, minimum, maximum]) => clamp(value, minimum, maximum));
    expect(expectedClamp).toMatchInlineSnapshot(`
      [
        5,
        0,
        10,
        0,
        NaN,
      ]
    `);
    expect([20, 0, -5, Number.NaN, Number.POSITIVE_INFINITY].map((duration) => normalizedDuration(duration))).toMatchInlineSnapshot(`
      [
        20,
        0,
        0,
        0,
        0,
      ]
    `);
    const windows: Array<[string, { startSeconds: number; endSeconds: number }]> = [
      ["inside", { startSeconds: 2, endSeconds: 6 }],
      ["overflowing", { startSeconds: 18, endSeconds: 24 }],
      ["negative", { startSeconds: -4, endSeconds: 3 }],
      ["wider than duration", { startSeconds: -5, endSeconds: 40 }],
      ["tiny", { startSeconds: 4, endSeconds: 4.1 }],
      ["inverted", { startSeconds: 6, endSeconds: 2 }],
      ["non-finite", { startSeconds: Number.NaN, endSeconds: Number.NaN }],
      ["fractional", { startSeconds: 1.23456, endSeconds: 3.98765 }],
    ];
    expect(
      [20, 0.1, 0].map((duration) => [
        duration,
        windows.map(([name, window]) => [name, normalizeWindow(window, duration)]),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          20,
          [
            [
              "inside",
              {
                "endSeconds": 6,
                "startSeconds": 2,
              },
            ],
            [
              "overflowing",
              {
                "endSeconds": 20,
                "startSeconds": 14,
              },
            ],
            [
              "negative",
              {
                "endSeconds": 7,
                "startSeconds": 0,
              },
            ],
            [
              "wider than duration",
              {
                "endSeconds": 20,
                "startSeconds": 0,
              },
            ],
            [
              "tiny",
              {
                "endSeconds": 4.25,
                "startSeconds": 4,
              },
            ],
            [
              "inverted",
              {
                "endSeconds": 6.25,
                "startSeconds": 6,
              },
            ],
            [
              "non-finite",
              {
                "endSeconds": 0.25,
                "startSeconds": 0,
              },
            ],
            [
              "fractional",
              {
                "endSeconds": 3.988,
                "startSeconds": 1.235,
              },
            ],
          ],
        ],
        [
          0.1,
          [
            [
              "inside",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
            [
              "overflowing",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
            [
              "negative",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
            [
              "wider than duration",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
            [
              "tiny",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
            [
              "inverted",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
            [
              "non-finite",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
            [
              "fractional",
              {
                "endSeconds": 0.1,
                "startSeconds": 0,
              },
            ],
          ],
        ],
        [
          0,
          [
            [
              "inside",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
            [
              "overflowing",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
            [
              "negative",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
            [
              "wider than duration",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
            [
              "tiny",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
            [
              "inverted",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
            [
              "non-finite",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
            [
              "fractional",
              {
                "endSeconds": 0,
                "startSeconds": 0,
              },
            ],
          ],
        ],
      ]
    `);
  });

  it("derives overview interaction windows", () => {
    const interaction = (mode: "pan" | "resizeStart" | "resizeEnd") => ({
      mode,
      pointerId: 1,
      pointerStartX: 100,
      overviewWidth: 400,
      window: { startSeconds: 4, endSeconds: 8 },
      moved: false,
    });
    const clientXs = [100, 180, -2000, 2000, 110.5];
    expect(
      (["pan", "resizeStart", "resizeEnd"] as const).map((mode) => [
        mode,
        clientXs.map((clientX) => [clientX, interactionWindow(interaction(mode), clientX, 20)]),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "pan",
          [
            [
              100,
              {
                "endSeconds": 8,
                "startSeconds": 4,
              },
            ],
            [
              180,
              {
                "endSeconds": 12,
                "startSeconds": 8,
              },
            ],
            [
              -2000,
              {
                "endSeconds": 4,
                "startSeconds": 0,
              },
            ],
            [
              2000,
              {
                "endSeconds": 20,
                "startSeconds": 16,
              },
            ],
            [
              110.5,
              {
                "endSeconds": 8.525,
                "startSeconds": 4.525,
              },
            ],
          ],
        ],
        [
          "resizeStart",
          [
            [
              100,
              {
                "endSeconds": 8,
                "startSeconds": 4,
              },
            ],
            [
              180,
              {
                "endSeconds": 8,
                "startSeconds": 7.75,
              },
            ],
            [
              -2000,
              {
                "endSeconds": 8,
                "startSeconds": 0,
              },
            ],
            [
              2000,
              {
                "endSeconds": 8,
                "startSeconds": 7.75,
              },
            ],
            [
              110.5,
              {
                "endSeconds": 8,
                "startSeconds": 4.525,
              },
            ],
          ],
        ],
        [
          "resizeEnd",
          [
            [
              100,
              {
                "endSeconds": 8,
                "startSeconds": 4,
              },
            ],
            [
              180,
              {
                "endSeconds": 12,
                "startSeconds": 4,
              },
            ],
            [
              -2000,
              {
                "endSeconds": 4.25,
                "startSeconds": 4,
              },
            ],
            [
              2000,
              {
                "endSeconds": 20,
                "startSeconds": 4,
              },
            ],
            [
              110.5,
              {
                "endSeconds": 8.525,
                "startSeconds": 4,
              },
            ],
          ],
        ],
      ]
    `);
    expect(interactionWindow(interaction("pan"), 180, 0)).toMatchInlineSnapshot(`null`);
    expect(interactionWindow(interaction("resizeEnd"), 120, 0.1)).toMatchInlineSnapshot(`
      {
        "endSeconds": 0.1,
        "startSeconds": 4,
      }
    `);
    expect(
      [
        [{ startSeconds: 1, endSeconds: 2 }, { startSeconds: 1.0005, endSeconds: 1.9995 }],
        [{ startSeconds: 1, endSeconds: 2 }, { startSeconds: 1.0006, endSeconds: 2 }],
        [{ startSeconds: 1, endSeconds: 2 }, { startSeconds: 1, endSeconds: 2.001 }],
      ].map(([first, second]) => windowsMatch(first!, second!)),
    ).toMatchInlineSnapshot(`
      [
        true,
        false,
        false,
      ]
    `);
  });

  it("lays out ruler badges", () => {
    const inputs = [
      { label: "00:00:05", seconds: 5, pixelsPerSecond: 80, visibleLeft: 0, visibleRight: 800 },
      { label: "00:00:00", seconds: 0, pixelsPerSecond: 80, visibleLeft: 200, visibleRight: 800 },
      { label: "00:00:30", seconds: 30, pixelsPerSecond: 80, visibleLeft: 0, visibleRight: 800 },
      { label: "a very long playhead badge label", seconds: 2, pixelsPerSecond: 80, visibleLeft: 0, visibleRight: 120 },
      { label: "00:00:01", seconds: 1, pixelsPerSecond: 80, visibleLeft: 500, visibleRight: 400 },
    ];
    expect(inputs.map((input) => timelineBadgeLayout(input))).toMatchInlineSnapshot(`
      [
        {
          "left": 400,
          "width": 62,
        },
        {
          "left": 200,
          "width": 62,
        },
        {
          "left": 738,
          "width": 62,
        },
        {
          "left": 0,
          "width": 120,
        },
        {
          "left": 500,
          "width": 0,
        },
      ]
    `);
    expect(
      [
        [{ left: 0, width: 50 }, { left: 100, width: 50 }],
        [{ left: 100, width: 50 }, { left: 0, width: 50 }],
        [{ left: 0, width: 50 }, { left: 40, width: 50 }],
        [{ left: 0, width: 50 }, { left: 50, width: 50 }],
      ].map(([first, second]) => renderedBadgeSeparation(first!, second!)),
    ).toMatchInlineSnapshot(`
      [
        50,
        50,
        0,
        0,
      ]
    `);
  });

  it("derives timeline item density and interaction timecodes", () => {
    expect(
      [0, 47.9, 48, 119.9, 120, 219.9, 220, 1000, Number.NaN].map((width) => [width, timelineItemDensity(width)]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          "accent",
        ],
        [
          47.9,
          "accent",
        ],
        [
          48,
          "title",
        ],
        [
          119.9,
          "title",
        ],
        [
          120,
          "timing",
        ],
        [
          219.9,
          "timing",
        ],
        [
          220,
          "rich",
        ],
        [
          1000,
          "rich",
        ],
        [
          NaN,
          "rich",
        ],
      ]
    `);
    expect(
      [0, 0.5, 1, 1.25, 59.999, 61, 3600, 7322.125, -1].map((seconds) => [seconds, formatInteractionTimecode(seconds)]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          "00:00:00.000",
        ],
        [
          0.5,
          "00:00:00.500",
        ],
        [
          1,
          "00:00:01.000",
        ],
        [
          1.25,
          "00:00:01.250",
        ],
        [
          59.999,
          "00:00:59.999",
        ],
        [
          61,
          "00:01:01.000",
        ],
        [
          3600,
          "01:00:00.000",
        ],
        [
          7322.125,
          "02:02:02.125",
        ],
        [
          -1,
          "00:00:00.000",
        ],
      ]
    `);
  });
});
