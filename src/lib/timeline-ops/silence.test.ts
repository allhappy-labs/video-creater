import { describe, expect, it } from "vitest";
import {
  currentTimelineSilenceRippleRange,
  rippleTrackIdsForItem,
  timelineSilenceRippleRanges,
} from "@/lib/timeline-ops/silence";
import type { VideoProject } from "@/lib/project";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";

function silenceProject(): VideoProject {
  const project = fixtureProject();
  const video = fixtureItem(project, "video");
  fixtureTrack(project, "video").items = [
    { ...video, id: "clip-main", startSeconds: 10, durationSeconds: 8, properties: { sourceIn: 2, sourceOut: 10 } },
    { ...video, id: "clip-no-range", startSeconds: 30, durationSeconds: 6, properties: {} },
    { ...video, id: "clip-other-media", startSeconds: 40, durationSeconds: 6, source: { type: "media", mediaId: "media-voiceover" }, properties: { sourceIn: 0 } },
    { ...video, id: "clip-generated", startSeconds: 50, durationSeconds: 6, source: { type: "generated", artifactId: "media-1" }, properties: {} },
  ];
  fixtureTrack(project, "overlay").items = [
    { ...video, id: "overlay-same-range", kind: "overlay", startSeconds: 10, durationSeconds: 8, properties: { sourceIn: 2, sourceOut: 10 } },
  ];
  project.mediaSilenceRanges = [
    // Fully inside clip-main's source range.
    { mediaId: "media-1", sourceIn: 3, sourceOut: 5, confidence: 0.9 },
    // Partially overlapping the start of clip-main's source range.
    { mediaId: "media-1", sourceIn: 1, sourceOut: 3.5, confidence: 0.8 },
    // Outside clip-main but inside clip-no-range.
    { mediaId: "media-1", sourceIn: 4.9, sourceOut: 5.5, confidence: 0.7 },
    { mediaId: "media-1", sourceIn: 20, sourceOut: 22, confidence: 0.7 },
    // Zero-length and invalid ranges.
    { mediaId: "media-1", sourceIn: 6, sourceOut: 6, confidence: 1 },
    { mediaId: "media-1", sourceIn: 7, sourceOut: 9, confidence: Number.NaN },
    { mediaId: "   ", sourceIn: 7, sourceOut: 9, confidence: 1 },
    // Exactly the minimum gap after padding, and the voiceover media.
    { mediaId: "media-1", sourceIn: 5.76, sourceOut: 6.75, confidence: 1 },
    { mediaId: "media-voiceover", sourceIn: 1, sourceOut: 3, confidence: 0.5 },
  ];
  return project;
}

describe("silence ripple characterization", () => {
  it("maps media silence to timeline ripple ranges", () => {
    const project = silenceProject();
    expect(timelineSilenceRippleRanges(project)).toMatchInlineSnapshot(`
      [
        {
          "endSeconds": 12.88,
          "startSeconds": 11.12,
          "trackIds": [
            "track-video",
          ],
        },
        {
          "endSeconds": 34.88,
          "startSeconds": 33.12,
          "trackIds": [
            "track-video",
          ],
        },
        {
          "endSeconds": 33.38,
          "startSeconds": 31.12,
          "trackIds": [
            "track-video",
          ],
        },
        {
          "endSeconds": 14.63,
          "startSeconds": 13.88,
          "trackIds": [
            "track-video",
          ],
        },
        {
          "endSeconds": 42.88,
          "startSeconds": 41.12,
          "trackIds": [
            "track-video",
          ],
        },
        {
          "endSeconds": 2.88,
          "startSeconds": 1.12,
          "trackIds": [
            "track-audio",
          ],
        },
      ]
    `);
    expect(timelineSilenceRippleRanges(fixtureProject())).toMatchInlineSnapshot(`[]`);
  });

  it("finds the silence range under the playhead", () => {
    const project = silenceProject();
    const playheads = [0, 11.12, 11.5, 12.88, 12.9, 30.5, 44, Number.NaN];
    expect(
      playheads.map((playhead) => [playhead, currentTimelineSilenceRippleRange(project, playhead)]),
    ).toMatchInlineSnapshot(`
      [
        [
          0,
          null,
        ],
        [
          11.12,
          {
            "endSeconds": 12.88,
            "startSeconds": 11.12,
            "trackIds": [
              "track-video",
            ],
          },
        ],
        [
          11.5,
          {
            "endSeconds": 12.88,
            "startSeconds": 11.12,
            "trackIds": [
              "track-video",
            ],
          },
        ],
        [
          12.88,
          null,
        ],
        [
          12.9,
          null,
        ],
        [
          30.5,
          null,
        ],
        [
          44,
          null,
        ],
        [
          NaN,
          null,
        ],
      ]
    `);
  });

  it("maps media silence through clip playback speed", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    fixtureTrack(project, "video").items = [
      // Source span 2..10 plays in 4 timeline seconds.
      { ...video, id: "clip-fast", startSeconds: 10, durationSeconds: 4, properties: { sourceIn: 2, sourceOut: 10, speed: 2 } },
      // Missing sourceOut: the implicit source span is duration * speed = 0..8.
      { ...video, id: "clip-fast-open", startSeconds: 30, durationSeconds: 4, properties: { speed: 2 } },
    ];
    fixtureTrack(project, "audio").items = [];
    project.mediaSilenceRanges = [
      { mediaId: "media-1", sourceIn: 3, sourceOut: 5, confidence: 0.9 },
      { mediaId: "media-1", sourceIn: 6, sourceOut: 7.5, confidence: 0.9 },
    ];

    expect(timelineSilenceRippleRanges(project)).toEqual([
      { startSeconds: 10.56, endSeconds: 11.44, trackIds: ["track-video"] },
      { startSeconds: 31.56, endSeconds: 32.44, trackIds: ["track-video"] },
      { startSeconds: 12.06, endSeconds: 12.69, trackIds: ["track-video"] },
      { startSeconds: 33.06, endSeconds: 33.69, trackIds: ["track-video"] },
    ]);
    expect(currentTimelineSilenceRippleRange(project, 12.5)).toEqual({
      startSeconds: 12.06,
      endSeconds: 12.69,
      trackIds: ["track-video"],
    });
  });

  it("maps media silence through reversed clips to where it plays", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    fixtureTrack(project, "video").items = [
      { ...video, id: "clip-reversed", startSeconds: 0, durationSeconds: 10, properties: { sourceIn: 0, sourceOut: 10, reverse: true } },
      { ...video, id: "clip-reversed-fast", startSeconds: 20, durationSeconds: 5, properties: { sourceIn: 0, sourceOut: 10, speed: 2, reverse: true } },
    ];
    fixtureTrack(project, "audio").items = [];
    // Source 1-2 s once the 0.12 s boundary padding is removed.
    project.mediaSilenceRanges = [{ mediaId: "media-1", sourceIn: 0.88, sourceOut: 2.12, confidence: 0.9 }];

    expect(timelineSilenceRippleRanges(project)).toEqual([
      { startSeconds: 8, endSeconds: 9, trackIds: ["track-video"] },
      { startSeconds: 24, endSeconds: 24.5, trackIds: ["track-video"] },
    ]);
  });

  it("collects ripple tracks for linked items", () => {
    const project = silenceProject();
    const video = fixtureItem(project, "video");
    fixtureItem(project, "audio").properties.linkGroupId = "link-1";
    const linked = { ...video, properties: { linkGroupId: "link-1" } };
    fixtureTrack(project, "overlay").items.push({ ...linked, id: "overlay-linked", startSeconds: 60 });
    expect([
      rippleTrackIdsForItem(project, video, "track-video"),
      rippleTrackIdsForItem(project, linked, "track-video"),
      rippleTrackIdsForItem(project, { ...video, properties: { linkGroupId: "  " } }, "track-source"),
      rippleTrackIdsForItem(project, { ...video, properties: { linkGroupId: "link-missing" } }, "track-source"),
    ]).toMatchInlineSnapshot(`
      [
        [
          "track-video",
        ],
        [
          "track-overlays",
          "track-audio",
        ],
        [
          "track-source",
        ],
        [],
      ]
    `);
  });
});
