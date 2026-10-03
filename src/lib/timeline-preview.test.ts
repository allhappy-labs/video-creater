import { describe, expect, it } from "vitest";
import {
  buildTimelinePreviewFrame,
  timelinePreviewLayerContainsPoint,
  topmostTimelinePreviewLayerAtPoint,
} from "./timeline-preview";
import type { GeneratedAsset, MediaAsset } from "./project";
import type { Timeline, TimelineItem, TimelineTrack, TransitionKind } from "./timeline";
import { transitionClipPath } from "./preview/transition-frame";
import { requiredAt } from "../test-utils/required";

const media: MediaAsset[] = [
  {
    id: "clip-1-media",
    relativePath: "media/clip-1.mp4",
    kind: "video",
    durationSeconds: 10,
    width: 1920,
    height: 1080,
    fps: 30,
    folderId: null,
  },
  {
    id: "image-1-media",
    relativePath: "media/still.png",
    kind: "image",
    durationSeconds: 4,
    width: 1280,
    height: 720,
    fps: null,
    folderId: null,
  },
];

const timeline: Timeline = {
  durationSeconds: 8,
  tracks: [
    {
      id: "track-video",
      kind: "video",
      name: "Video",
      locked: false,
      enabled: true,
      items: [
        {
          id: "clip-1",
          kind: "video_clip",
          startSeconds: 1,
          durationSeconds: 3,
          label: "Opening",
          source: { type: "media", mediaId: "clip-1-media" },
          properties: { sourceIn: 2, sourceOut: 5, opacity: 0.8 },
        },
        {
          id: "image-1",
          kind: "video_clip",
          startSeconds: 5,
          durationSeconds: 2,
          label: "Still",
          source: { type: "media", mediaId: "image-1-media" },
          properties: {},
        },
      ],
    },
  ],
};

const generatedAssets: GeneratedAsset[] = [
  {
    schemaVersion: 1,
    id: "generated-shot-1",
    kind: "generated",
    status: "completed",
    name: "Generated shot",
    targetFolderId: null,
    placementIntent: "timeline",
    prompt: "A generated skyline shot",
    model: { provider: "mock", id: "video" },
    references: {
      mediaIds: [],
      firstFrameMediaId: null,
      lastFrameMediaId: null,
    },
    settings: {
      width: 1280,
      height: 720,
      durationSeconds: 4,
      fps: 24,
      aspectRatio: "16:9",
    },
    outputs: [
      {
        mediaId: "generated-shot-1-output",
        relativePath: "generated/shot-1.mp4",
        width: 1280,
        height: 720,
        durationSeconds: 4,
        fps: 24,
      },
    ],
    createdAt: "2026-07-01T00:00:00Z",
    parentAssetId: null,
    retryOfAssetId: null,
  },
];

const primaryMedia = requiredAt(media, 0, "primary preview media");
const videoTrack = requiredAt(timeline.tracks, 0, "video track");
const openingItem = requiredAt(videoTrack.items, 0, "opening item");

describe("timeline preview frame", () => {
  it("returns active media layers at the playhead", () => {
    const frame = buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 2 });

    expect(frame.layers).toHaveLength(1);
    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      itemId: "clip-1",
      mediaId: "clip-1-media",
      mediaKind: "video",
      sourceTimeSeconds: 3,
      opacity: 0.8,
    });
  });

  it("evaluates media opacity and transform keyframes at the playhead", () => {
    const keyframed: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              properties: {
                keyframes: {
                  opacity: [
                    { atSeconds: 0, value: 0.2 },
                    { atSeconds: 2, value: 1 },
                  ],
                  positionX: [
                    { atSeconds: 0, value: -100 },
                    { atSeconds: 2, value: 100 },
                  ],
                  positionY: [
                    { atSeconds: 0, value: 20 },
                    { atSeconds: 2, value: -20 },
                  ],
                  scale: [
                    { atSeconds: 0, value: 1 },
                    { atSeconds: 2, value: 1.5 },
                  ],
                  scaleX: [
                    { atSeconds: 0, value: 1 },
                    { atSeconds: 2, value: 0.75 },
                  ],
                  scaleY: [
                    { atSeconds: 0, value: 1 },
                    { atSeconds: 2, value: 0.5 },
                  ],
                  rotationDegrees: [
                    { atSeconds: 0, value: -10 },
                    { atSeconds: 2, value: 10 },
                  ],
                },
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: keyframed,
      media,
      playheadSeconds: 2,
    });

    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      opacity: 0.6,
      positionX: 0,
      positionY: 0,
      scale: 1.25,
      scaleX: 0.875,
      scaleY: 0.75,
      rotationDegrees: 0,
    });
  });

  describe("static motion properties", () => {
    function frameWithOpeningProperties(properties: Record<string, unknown>) {
      const withProperties: Timeline = {
        ...timeline,
        tracks: [{ ...videoTrack, items: [{ ...openingItem, properties: { sourceIn: 2, sourceOut: 5, ...properties } }] }],
      };
      return requiredAt(buildTimelinePreviewFrame({ timeline: withProperties, media, playheadSeconds: 2 }).layers, 0, "preview layer");
    }

    it("applies a static rotation when rotationDegrees has no keyframes", () => {
      expect(frameWithOpeningProperties({ rotationDegrees: 15 })).toMatchObject({ rotationDegrees: 15 });
    });

    it("applies static scale and position, with scale feeding both axes", () => {
      expect(frameWithOpeningProperties({ scale: 1.5, positionX: 120, positionY: -40 })).toMatchObject({
        scale: 1.5,
        scaleX: 1.5,
        scaleY: 1.5,
        positionX: 120,
        positionY: -40,
      });
    });

    it("prefers a static per-axis scale over the uniform scale", () => {
      expect(frameWithOpeningProperties({ scale: 1.5, scaleX: 0.5 })).toMatchObject({ scale: 1.5, scaleX: 0.5, scaleY: 1.5 });
    });

    it("lets keyframes win over static values", () => {
      const layer = frameWithOpeningProperties({
        scale: 3,
        positionX: 500,
        rotationDegrees: 45,
        keyframes: {
          scale: [
            { atSeconds: 0, value: 1 },
            { atSeconds: 2, value: 2 },
          ],
          positionX: [{ atSeconds: 0, value: 10 }],
          rotationDegrees: [{ atSeconds: 0, value: -5 }],
        },
      });
      expect(layer).toMatchObject({ scale: 1.5, scaleX: 1.5, scaleY: 1.5, positionX: 10, rotationDegrees: -5 });
    });

    it("keeps the identity motion when no motion properties are set", () => {
      expect(frameWithOpeningProperties({})).toMatchObject({
        positionX: 0,
        positionY: 0,
        scale: 1,
        scaleX: 1,
        scaleY: 1,
        rotationDegrees: 0,
      });
    });
  });

  it("maps validated basic color-grade controls into the preview layer", () => {
    const graded: Timeline = {
      ...timeline,
      tracks: [{
        ...videoTrack,
        items: [{
          ...openingItem,
          properties: {
            ...openingItem.properties,
            colorGrade: { exposure: 1, contrast: 1.25, saturation: 0.8 },
          },
        }],
      }],
    };

    const gradedLayer = requiredAt(
      buildTimelinePreviewFrame({ timeline: graded, media, playheadSeconds: 2 }).layers,
      0,
      "graded preview layer",
    );
    expect(gradedLayer.colorGrade).toEqual({
      exposure: 1,
      contrast: 1.25,
      saturation: 0.8,
    });
  });

  it("maps reviewed visual blend modes into the preview layer", () => {
    const blended: Timeline = {
      ...timeline,
      tracks: [{
        ...videoTrack,
        items: [{
          ...openingItem,
          properties: {
            ...openingItem.properties,
            blendMode: "add",
          },
        }],
      }],
    };

    const blendedLayer = requiredAt(
      buildTimelinePreviewFrame({ timeline: blended, media, playheadSeconds: 2 }).layers,
      0,
      "blended preview layer",
    );
    expect(blendedLayer.blendMode).toBe("add");
    expect(blendedLayer.canonicalPreparationRequired).toBe(true);
  });

  it("requires canonical preparation for executable visual effect stacks", () => {
    const effected: Timeline = {
      ...timeline,
      tracks: [{
        ...videoTrack,
        items: [{
          ...openingItem,
          properties: {
            ...openingItem.properties,
            effects: [{ effectType: "stylize.grain", enabled: true, params: { amount: 0.2 } }],
          },
        }],
      }],
    };
    const effectedLayer = requiredAt(
      buildTimelinePreviewFrame({ timeline: effected, media, playheadSeconds: 2 }).layers,
      0,
      "effected preview layer",
    );
    expect(effectedLayer.canonicalPreparationRequired).toBe(true);
  });

  it("requires canonical preparation for reversed video and audio clips", () => {
    const audioMedia: MediaAsset = { ...primaryMedia, id: "music-media", relativePath: "media/music.wav", kind: "audio" };
    const reversed: Timeline = {
      ...timeline,
      tracks: [
        { ...videoTrack, items: [{ ...openingItem, properties: { ...openingItem.properties, reverse: true } }] },
        {
          id: "track-audio",
          kind: "audio",
          name: "Audio",
          locked: false,
          enabled: true,
          items: [
            {
              id: "music-bed",
              kind: "audio_clip",
              startSeconds: 1,
              durationSeconds: 3,
              label: "Music bed",
              source: { type: "media", mediaId: "music-media" },
              properties: { sourceIn: 2, sourceOut: 5, reverse: true },
            },
          ],
        },
      ],
    };
    const frame = buildTimelinePreviewFrame({ timeline: reversed, media: [...media, audioMedia], playheadSeconds: 2 });
    expect(requiredAt(frame.layers, 0, "reversed video layer")).toMatchObject({ itemId: "clip-1", canonicalPreparationRequired: true });
    expect(requiredAt(frame.audioLayers, 0, "reversed audio layer")).toMatchObject({
      itemId: "music-bed",
      sourceTimeSeconds: 4,
      canonicalPreparationRequired: true,
    });

    const forward = buildTimelinePreviewFrame({
      timeline: { ...reversed, tracks: reversed.tracks.map((track) => ({ ...track, items: track.items.map((entry) => ({ ...entry, properties: { ...entry.properties, reverse: false } })) })) },
      media: [...media, audioMedia],
      playheadSeconds: 2,
    });
    expect(forward.layers.map((layer) => layer.canonicalPreparationRequired)).toEqual([false]);
    expect(forward.audioLayers.map((layer) => layer.canonicalPreparationRequired)).toEqual([false]);
  });

  it("multiplies visual opacity by canonical fade-in and fade-out envelopes", () => {
    const faded: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              properties: {
                ...openingItem.properties,
                opacity: 0.8,
                fadeInSeconds: 1,
                fadeOutSeconds: 1,
              },
            },
          ],
        },
      ],
    };

    const fadeInLayer = requiredAt(
      buildTimelinePreviewFrame({ timeline: faded, media, playheadSeconds: 1.5 }).layers,
      0,
      "fade-in preview layer",
    );
    const fadeOutLayer = requiredAt(
      buildTimelinePreviewFrame({ timeline: faded, media, playheadSeconds: 3.5 }).layers,
      0,
      "fade-out preview layer",
    );
    expect(fadeInLayer.opacity).toBe(0.4);
    expect(fadeOutLayer.opacity).toBe(0.4);
  });

  it("reports empty preview when no item is active", () => {
    const frame = buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 4.5 });

    expect(frame.layers).toEqual([]);
    expect(frame.status).toBe("empty");
  });

  it("reports missing media for broken timeline sources", () => {
    const broken: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              source: { type: "media", mediaId: "missing-media" },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({ timeline: broken, media, playheadSeconds: 2 });

    expect(frame.status).toBe("missing-media");
    expect(frame.issues[0]).toContain("missing-media");
  });

  it("resolves completed generated timeline sources to previewable output media", () => {
    const generatedTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              id: "generated-clip-1",
              kind: "video_clip",
              startSeconds: 0,
              durationSeconds: 3,
              label: "Generated shot",
              source: { type: "generated", artifactId: "generated-shot-1" },
              properties: { sourceIn: 0.5 },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: generatedTimeline,
      media,
      generatedAssets,
      playheadSeconds: 1,
    });

    expect(frame.status).toBe("ready");
    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      itemId: "generated-clip-1",
      mediaId: "generated-shot-1-output",
      mediaKind: "generated",
      relativePath: "generated/shot-1.mp4",
      sourceTimeSeconds: 1.5,
    });
  });

  it("reports unsupported preview sources with blank media paths", () => {
    const blankPathMedia: MediaAsset[] = [
      {
        ...primaryMedia,
        relativePath: "   ",
      },
    ];

    const frame = buildTimelinePreviewFrame({
      timeline,
      media: blankPathMedia,
      playheadSeconds: 2,
    });

    expect(frame.status).toBe("unsupported-source");
    expect(frame.layers).toEqual([]);
    expect(frame.issues[0]).toContain("has no preview path");
  });

  it("expands nested timeline sources at the shared playhead", () => {
    const nested = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              id: "nested-opening",
              label: "Nested opening",
              startSeconds: 0.5,
              durationSeconds: 2,
            },
          ],
        },
      ],
    };
    const frame = buildTimelinePreviewFrame({
      timeline: {
        ...timeline,
        tracks: [
          {
            ...videoTrack,
            items: [
              {
                ...openingItem,
                source: { type: "timeline", timelineId: "alternate" },
                startSeconds: 1,
                durationSeconds: 3,
                properties: { opacity: 0.5 },
              },
            ],
          },
        ],
      },
      timelines: [{ id: "alternate", timeline: nested }],
      media,
      playheadSeconds: 2,
    });

    expect(frame.status).toBe("ready");
    expect(frame.issues).toEqual([]);
    expect(frame.layers).toHaveLength(1);
    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      itemId: "root:0:clip-1:nested-opening",
      label: "Nested opening",
      timelineStartSeconds: 1.5,
      sourceTimeSeconds: 2.5,
      opacity: 0.4,
    });
  });

  it("retimes nested preview layers with wrapper speed", () => {
    const nested: Timeline = {
      ...timeline,
      tracks: [{
        ...videoTrack,
        items: [{
          ...openingItem,
          id: "nested-speed",
          startSeconds: 0.5,
          durationSeconds: 2,
        }],
      }],
    };
    const frame = buildTimelinePreviewFrame({
      timeline: {
        ...timeline,
        tracks: [{
          ...videoTrack,
          items: [{
            ...openingItem,
            source: { type: "timeline", timelineId: "alternate-speed" },
            startSeconds: 1,
            durationSeconds: 2,
            properties: { speed: 2 },
          }],
        }],
      },
      timelines: [{ id: "alternate-speed", timeline: nested }],
      media,
      playheadSeconds: 1.5,
    });
    expect(frame.status).toBe("ready");
    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      itemId: "root:0:clip-1:nested-speed",
      timelineStartSeconds: 1.25,
      timelineEndSeconds: 2.25,
      sourceTimeSeconds: 2.5,
    });
  });

  it("composes nested wrapper motion keyframes into child preview motion", () => {
    const nested: Timeline = {
      ...timeline,
      tracks: [{ ...videoTrack, items: [{
        ...openingItem, id: "nested-motion", startSeconds: 0, durationSeconds: 3,
        properties: { ...openingItem.properties, positionX: 10 },
      }] }],
    };
    const frame = buildTimelinePreviewFrame({
      timeline: { ...timeline, tracks: [{ ...videoTrack, items: [{
        ...openingItem, source: { type: "timeline", timelineId: "motion" },
        startSeconds: 1, durationSeconds: 3,
        properties: { keyframes: { positionX: [
          { atSeconds: 0, value: 0, easing: "linear" },
          { atSeconds: 3, value: 90, easing: "linear" },
        ] } },
      }] }] },
      timelines: [{ id: "motion", timeline: nested }], media, playheadSeconds: 2,
    });
    expect(frame.status).toBe("ready");
    expect(requiredAt(frame.layers, 0, "preview layer").positionX).toBe(40);
  });

  it("composes nested wrapper and child opacity keyframes at the playhead", () => {
    const nested: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              id: "nested-opacity-opening",
              startSeconds: 0,
              durationSeconds: 3,
              properties: {
                opacity: 0.8,
                keyframes: {
                  opacity: [
                    { atSeconds: 0, value: 0.8 },
                    { atSeconds: 3, value: 0.4 },
                  ],
                },
              },
            },
          ],
        },
      ],
    };
    const frame = buildTimelinePreviewFrame({
      timeline: {
        ...timeline,
        tracks: [
          {
            ...videoTrack,
            items: [
              {
                ...openingItem,
                source: { type: "timeline", timelineId: "alternate" },
                startSeconds: 1,
                durationSeconds: 3,
                properties: {
                  keyframes: {
                    opacity: [
                      { atSeconds: 0, value: 0.5 },
                      { atSeconds: 3, value: 0.25 },
                    ],
                  },
                },
              },
            ],
          },
        ],
      },
      timelines: [{ id: "alternate", timeline: nested }],
      media,
      playheadSeconds: 2.5,
    });

    expect(frame.status).toBe("ready");
    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      itemId: "root:0:clip-1:nested-opacity-opening",
      opacity: 0.225,
    });
  });

  it("composes nested wrapper fades at the shared playhead", () => {
    const nested: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              id: "nested-fade-opening",
              startSeconds: 0,
              durationSeconds: 3,
              properties: { opacity: 0.8 },
            },
          ],
        },
      ],
    };
    const frame = buildTimelinePreviewFrame({
      timeline: {
        ...timeline,
        tracks: [
          {
            ...videoTrack,
            items: [
              {
                ...openingItem,
                source: { type: "timeline", timelineId: "alternate" },
                startSeconds: 1,
                durationSeconds: 3,
                properties: { fadeInSeconds: 1, fadeOutSeconds: 1 },
              },
            ],
          },
        ],
      },
      timelines: [{ id: "alternate", timeline: nested }],
      media,
      playheadSeconds: 1.5,
    });

    expect(frame.status).toBe("ready");
    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      itemId: "root:0:clip-1:nested-fade-opening",
      opacity: 0.4,
    });
  });

  it("composes a nested wrapper canvas transform into preview layers", () => {
    const nested = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              id: "nested-transformed-opening",
              properties: {
                ...openingItem.properties,
                transform: { centerX: 0.2, centerY: 0.5, width: 0.5, height: 1, flipHorizontal: true },
              },
            },
          ],
        },
      ],
    };
    const frame = buildTimelinePreviewFrame({
      timeline: {
        ...timeline,
        tracks: [
          {
            ...videoTrack,
            items: [
              {
                ...openingItem,
                source: { type: "timeline", timelineId: "alternate" },
                properties: {
                  transform: { centerX: 0.25, centerY: 0.5, width: 0.5, height: 1, flipHorizontal: true },
                },
              },
            ],
          },
        ],
      },
      timelines: [{ id: "alternate", timeline: nested }],
      media,
      playheadSeconds: 2.5,
    });

    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      centerX: 0.4,
      centerY: 0.5,
      width: 0.25,
      height: 1,
      flipHorizontal: false,
      flipVertical: false,
    });
  });

  it("composes nested wrapper audio gain into preview audio layers", () => {
    const audioMedia: MediaAsset[] = [
      ...media,
      {
        id: "voiceover-media",
        relativePath: "media/voiceover.wav",
        kind: "audio",
        durationSeconds: 4,
        width: null,
        height: null,
        fps: null,
        folderId: null,
      },
    ];
    const nested: Timeline = {
      durationSeconds: 4,
      tracks: [
        {
          id: "nested-audio",
          kind: "audio",
          name: "Voiceover",
          locked: false,
          enabled: true,
          items: [
            {
              id: "nested-voiceover",
              kind: "audio_clip",
              startSeconds: 0,
              durationSeconds: 4,
              label: "Voiceover",
              source: { type: "media", mediaId: "voiceover-media" },
              properties: { volumeDb: -3 },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: {
        durationSeconds: 4,
        tracks: [
          {
            id: "root-video",
            kind: "video",
            name: "Nested sequence",
            locked: false,
            enabled: true,
            items: [
              {
                id: "nested-wrapper",
                kind: "video_clip",
                startSeconds: 0,
                durationSeconds: 4,
                label: "Nested sequence",
                source: { type: "timeline", timelineId: "alternate" },
                properties: { volumeDb: -6 },
              },
            ],
          },
        ],
      },
      timelines: [{ id: "alternate", timeline: nested }],
      media: audioMedia,
      playheadSeconds: 1,
    });

    expect(frame.status).toBe("ready");
    expect(frame.audioLayers).toHaveLength(1);
    expect(frame.audioLayers.at(0)).toMatchObject({
      itemId: "root:0:nested-wrapper:nested-voiceover",
      gain: 0.355,
    });
  });

  it("clamps source time to the source range and reports the preview issue", () => {
    const trimmed: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              durationSeconds: 5,
              properties: { sourceIn: 8, sourceOut: 10 },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: trimmed,
      media,
      playheadSeconds: 5.5,
    });

    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      sourceTimeSeconds: 10,
    });
    expect(frame.issues[0]).toContain("outside its preview source range");
  });

  it("surfaces caption placement in the shared preview layer evaluator", () => {
    const captions: Timeline = {
      durationSeconds: 2,
      tracks: [
        {
          id: "track-captions",
          kind: "caption",
          name: "Captions",
          locked: false,
          enabled: true,
          items: [
            {
              id: "caption-upper",
              kind: "caption",
              startSeconds: 0,
              durationSeconds: 2,
              label: "Upper caption",
              source: { type: "text", text: "Keep this clear" },
              properties: {
                captionPlacement: "upper",
                stylePreset: "kineticFocus",
                emphasizedWordIndices: [1, 99, 1],
                captionWordTimings: [{ wordIndex: 1, startSeconds: 0.5, endSeconds: 1.5 }],
                captionWordAnimations: [{ wordIndex: 1, enterStartSeconds: 0.5, enterEndSeconds: 0.7, holdEndSeconds: 1.2, exitEndSeconds: 1.5, emphasisScale: 1.2, emphasisColor: "#ff3355", emphasisOpacity: 0.9, easing: "outBack" }],
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: captions,
      media,
      playheadSeconds: 1,
    });

    expect(frame.overlayLayers).toMatchObject([
      {
        itemId: "caption-upper",
        captionPlacement: "upper",
        captionStylePreset: "kineticFocus",
        emphasizedWordIndices: [1],
        activeEmphasizedWordIndices: [1],
        captionWordStyles: [{ wordIndex: 1, scale: 1.2, opacity: 0.9, color: "#ff3355" }],
      },
    ]);
  });

  it("surfaces enabled grain and vignette finishing effects for the preview compositor", () => {
    const effectsTimeline: Timeline = {
      ...timeline,
      tracks: timeline.tracks.map((track) => ({
        ...track,
        items: track.items.map((item) =>
          item.id === "clip-1"
            ? {
                ...item,
                properties: {
                  ...item.properties,
                  effects: [
                    { effectType: "stylize.grain", enabled: true, params: { amount: 0.18 } },
                    { effectType: "stylize.vignette", enabled: true, params: { amount: -0.25 } },
                  ],
                },
              }
            : item,
        ),
      })),
    };

    const frame = buildTimelinePreviewFrame({
      timeline: effectsTimeline,
      media,
      playheadSeconds: 1.5,
    });

    expect(requiredAt(frame.layers, 0, "preview layer")).toMatchObject({
      effects: { grain: true, vignette: true },
    });
  });

  it("returns active caption, text overlay, and template overlay layers", () => {
    const layeredTimeline: Timeline = {
      ...timeline,
      tracks: [
        ...timeline.tracks,
        {
          id: "track-overlays",
          kind: "overlay",
          name: "Overlays",
          locked: false,
          enabled: true,
          items: [
            {
              id: "overlay-1",
              kind: "overlay",
              startSeconds: 1.5,
              durationSeconds: 2,
              label: "Launch now",
              source: { type: "text", text: "Launch now" },
              properties: { opacity: 0.7 },
            },
            {
              id: "template-1",
              kind: "overlay",
              startSeconds: 1,
              durationSeconds: 3,
              label: "Lower third",
              source: { type: "text", text: "" },
              properties: { templateId: "kinetic-lower-third-v1" },
            },
          ],
        },
        {
          id: "track-captions",
          kind: "caption",
          name: "Captions",
          locked: false,
          enabled: true,
          items: [
            {
              id: "caption-1",
              kind: "caption",
              startSeconds: 1.25,
              durationSeconds: 2,
              label: "Caption",
              source: { type: "text", text: "Welcome back" },
              properties: {},
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: layeredTimeline,
      media,
      playheadSeconds: 2,
    });

    expect(frame.overlayLayers).toEqual([
      expect.objectContaining({
        itemId: "overlay-1",
        overlayKind: "text",
        text: "Launch now",
        opacity: 0.7,
      }),
      expect.objectContaining({
        itemId: "template-1",
        overlayKind: "template",
        templateId: "kinetic-lower-third-v1",
      }),
      expect.objectContaining({
        itemId: "caption-1",
        overlayKind: "caption",
        text: "Welcome back",
      }),
    ]);
  });

  it("evaluates overlay opacity and transform keyframes at the playhead", () => {
    const overlayTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-overlays",
          kind: "overlay",
          name: "Overlays",
          locked: false,
          enabled: true,
          items: [
            {
              id: "overlay-1",
              kind: "overlay",
              startSeconds: 1,
              durationSeconds: 4,
              label: "Launch now",
              source: { type: "text", text: "Launch now" },
              properties: {
                keyframes: {
                  opacity: [
                    { atSeconds: 0, value: 0 },
                    { atSeconds: 2, value: 1 },
                  ],
                  positionX: [
                    { atSeconds: 0, value: 0 },
                    { atSeconds: 2, value: 48 },
                  ],
                  scale: [
                    { atSeconds: 0, value: 0.9 },
                    { atSeconds: 2, value: 1.1 },
                  ],
                  rotationDegrees: [
                    { atSeconds: 0, value: -4 },
                    { atSeconds: 2, value: 4 },
                  ],
                },
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: overlayTimeline,
      media,
      playheadSeconds: 2,
    });

    expect(requiredAt(frame.overlayLayers, 0, "overlay layer")).toMatchObject({
      opacity: 0.5,
      positionX: 24,
      positionY: 0,
      scale: 1,
      rotationDegrees: 0,
    });
  });

  it("treats generated-source template items as overlay layers", () => {
    const templateTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-overlays",
          kind: "overlay",
          name: "Overlays",
          locked: false,
          enabled: true,
          items: [
            {
              id: "template-1",
              kind: "overlay",
              startSeconds: 0,
              durationSeconds: 2.4,
              label: "Kinetic Lower Third",
              source: {
                type: "generated",
                artifactId: "template:kinetic-lower-third-v1:template-1",
              },
              properties: {
                templateId: "kinetic-lower-third-v1",
                templateFields: {
                  headline: "Olha API",
                  subline: "Founder",
                },
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: templateTimeline,
      media,
      generatedAssets,
      playheadSeconds: 1,
    });

    expect(frame.status).toBe("ready");
    expect(frame.layers).toEqual([]);
    expect(frame.overlayLayers).toEqual([
      expect.objectContaining({
        itemId: "template-1",
        overlayKind: "template",
        templateId: "kinetic-lower-third-v1",
      }),
    ]);
    expect(frame.issues).toEqual([]);
  });

  it("previews supported HyperFrame scene kinds with backend-aligned default templates", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-title",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Title",
              source: { type: "text", text: "Title" },
              properties: { kind: "title_card" },
            },
            {
              id: "scene-diagram",
              kind: "hyperframe_scene",
              startSeconds: 1,
              durationSeconds: 1,
              label: "Diagram",
              source: { type: "text", text: "Diagram" },
              properties: { kind: "diagram" },
            },
            {
              id: "scene-lower-third",
              kind: "hyperframe_scene",
              startSeconds: 2,
              durationSeconds: 1,
              label: "Lower third",
              source: { type: "text", text: "Olha API" },
              properties: { kind: "lower_third" },
            },
            {
              id: "scene-transition",
              kind: "hyperframe_scene",
              startSeconds: 3,
              durationSeconds: 1,
              label: "Transition",
              source: { type: "text", text: "Transition" },
              properties: { kind: "transition" },
            },
            {
              id: "scene-immersive",
              kind: "hyperframe_scene",
              startSeconds: 4,
              durationSeconds: 1,
              label: "Immersive",
              source: { type: "text", text: "Immersive" },
              properties: { kind: "immersive_scene" },
            },
          ],
        },
      ],
    };

    const templateIds = [0.5, 1.5, 2.5, 3.5, 4.5].map((playheadSeconds) => {
      const frame = buildTimelinePreviewFrame({
        timeline: hyperframeTimeline,
        media,
        playheadSeconds,
      });

      expect(frame.status).toBe("ready");
      expect(frame.layers).toEqual([]);
      expect(frame.overlayLayers).toHaveLength(1);
      expect(frame.issues).toEqual([]);
      return requiredAt(frame.overlayLayers, 0, "overlay layer").templateId;
    });

    expect(templateIds).toEqual([
      "chapter-card-v1",
      "metric-callout-v1",
      "kinetic-lower-third-v1",
      "gradient-background-loop-v1",
      "gradient-background-loop-v1",
    ]);
  });

  it("previews logo immersive HyperFrame scenes with the holographic default template", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-logo",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Brand reveal",
              source: { type: "text", text: "Brand reveal" },
              properties: {
                kind: "immersive_scene",
                sourceBeat: "Reveal the product brand before the payoff",
                visualTreatment:
                  "full-frame holographic logo cutout with pearlescent shader bands",
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: hyperframeTimeline,
      media,
      playheadSeconds: 0.5,
    });

    expect(frame.status).toBe("ready");
    expect(frame.overlayLayers).toHaveLength(1);
    expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe("holographic-logo-cutout-v1");
  });

  it("previews metric immersive HyperFrame scenes with the metric callout template", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-metric",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Retention metric",
              source: { type: "text", text: "42% higher retention" },
              properties: {
                kind: "immersive_scene",
                sourceBeat: "Show the key data point before the product proof",
                visualTreatment:
                  "floating metric tile with high-contrast number and directional accent",
                motion: "metric count-up feel, accent sweep, hold, then slide out",
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: hyperframeTimeline,
      media,
      playheadSeconds: 0.5,
    });

    expect(frame.status).toBe("ready");
    expect(frame.overlayLayers).toHaveLength(1);
    expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe("metric-callout-v1");
  });

  it("previews detail immersive HyperFrame scenes with the tracking highlight template", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-detail",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Product detail callout",
              source: { type: "text", text: "Look here" },
              properties: {
                kind: "immersive_scene",
                sourceBeat: "Direct attention to the visual detail that proves the claim",
                visualTreatment: "thin tracking ring with compact label and pointer line",
                motion: "tracking highlight draws on, label slides from pointer, then fades",
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: hyperframeTimeline,
      media,
      playheadSeconds: 0.5,
    });

    expect(frame.status).toBe("ready");
    expect(frame.overlayLayers).toHaveLength(1);
    expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe("tracking-highlight-v1");
  });

  it("previews quote immersive HyperFrame scenes with the punchy caption template", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-quote",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Key quote",
              source: { type: "text", text: "This changes everything" },
              properties: {
                kind: "immersive_scene",
                sourceBeat: "Emphasize the key quote before the payoff cut",
                visualTreatment:
                  "large phone-readable quote caption lockup with accent underline and soft backing",
                motion: "snap pop in, underline wipe, hold, then quick fade",
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: hyperframeTimeline,
      media,
      playheadSeconds: 0.5,
    });

    expect(frame.status).toBe("ready");
    expect(frame.overlayLayers).toHaveLength(1);
    expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe("punchy-caption-v1");
  });

  it("previews chapter immersive HyperFrame scenes with the chapter card template", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-chapter",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Story chapter reset",
              source: { type: "text", text: "Chapter 02" },
              properties: {
                kind: "immersive_scene",
                sourceBeat: "Introduce the next chapter of the story before the payoff",
                visualTreatment:
                  "left-weighted chapter marker with translucent panel and vertical reveal line",
                motion: "vertical line wipe, text type-on, short hold, then mask out",
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: hyperframeTimeline,
      media,
      playheadSeconds: 0.5,
    });

    expect(frame.status).toBe("ready");
    expect(frame.overlayLayers).toHaveLength(1);
    expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe("chapter-card-v1");
  });

  it("previews comparison immersive HyperFrame scenes with the metric callout template", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-comparison",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Before vs after comparison",
              source: { type: "text", text: "Before vs After" },
              properties: {
                kind: "immersive_scene",
                sourceBeat: "Compare the old workflow against the new outcome before the proof cut",
                visualTreatment:
                  "split comparison tile with two concise labels and a directional accent",
                motion: "left label enters, right label counters, accent sweep bridges the two states",
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: hyperframeTimeline,
      media,
      playheadSeconds: 0.5,
    });

    expect(frame.status).toBe("ready");
    expect(frame.overlayLayers).toHaveLength(1);
    expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe("metric-callout-v1");
  });

  it("previews process immersive HyperFrame scenes with the chapter card template", () => {
    const hyperframeTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-scenes",
          kind: "hyperframe_scene",
          name: "HyperFrames",
          locked: false,
          enabled: true,
          items: [
            {
              id: "scene-process",
              kind: "hyperframe_scene",
              startSeconds: 0,
              durationSeconds: 1,
              label: "Three-step rollout map",
              source: { type: "text", text: "Plan -> Build -> Review" },
              properties: {
                kind: "immersive_scene",
                sourceBeat: "Explain the rollout process before the final proof cut",
                visualTreatment:
                  "staged roadmap with three milestone markers and a vertical reveal line",
                motion: "step markers reveal in sequence, connector line wipes through the process",
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: hyperframeTimeline,
      media,
      playheadSeconds: 0.5,
    });

    expect(frame.status).toBe("ready");
    expect(frame.overlayLayers).toHaveLength(1);
    expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe("chapter-card-v1");
  });

  it.each([
    {
      id: "scene-testimonial",
      label: "Customer testimonial interview",
      sourceBeat: "Lift the customer testimonial into a phone-readable interview beat",
      visualTreatment: "testimonial lockup with speaker treatment and compact backing",
      motion: "testimonial text snaps in, speaker line wipes, then fades",
      expectedTemplateId: "punchy-caption-v1",
    },
    {
      id: "scene-feature",
      label: "Product feature spec closeup",
      sourceBeat: "Show the product feature and spec before the proof cut",
      visualTreatment: "precise feature lens with leader line and compact spec label",
      motion: "feature lens draws in, spec label slides from the edge, then clears",
      expectedTemplateId: "tracking-highlight-v1",
    },
    {
      id: "scene-launch",
      label: "Launch announcement beat",
      sourceBeat: "Introduce the launch announcement before the reveal",
      visualTreatment: "editorial announcement marker with vertical reveal and subline",
      motion: "announcement line wipes up, headline reveals, short hold, then masks out",
      expectedTemplateId: "chapter-card-v1",
    },
    {
      id: "scene-route",
      label: "Route map transition",
      sourceBeat: "Show the location route before the arrival reveal",
      visualTreatment: "animated city map with a place marker and route line",
      motion: "route line draws across the map, place marker pulses, text slides in",
      expectedTemplateId: "tracking-highlight-v1",
    },
    {
      id: "scene-reaction",
      label: "Surprised reaction beat",
      sourceBeat: "Show the emotional reaction before the next cut",
      visualTreatment: "large reaction typography with expressive accent stroke",
      motion: "reaction word pops on, accent stroke snaps, then clears quickly",
      expectedTemplateId: "punchy-caption-v1",
    },
    {
      id: "scene-pricing",
      label: "Buyer proof beat",
      sourceBeat: "Show the buyer proof before the close",
      visualTreatment: "compact proof tile with directional accent",
      motion: "figure counts up, delta sweeps, then settles",
      fields: {
        headline: "Pricing savings",
        subline: "Budget impact and ROI",
      },
      expectedTemplateId: "metric-callout-v1",
    },
    {
      id: "scene-risk",
      label: "Compliance risk warning",
      sourceBeat: "Flag the security risk before the remediation step",
      visualTreatment: "urgent warning caption with compact compliance marker",
      motion: "alert word snaps on, warning rule wipes, then clears",
      expectedTemplateId: "punchy-caption-v1",
    },
    {
      id: "scene-deadline",
      label: "Deadline schedule beat",
      sourceBeat: "Show the calendar deadline and due date",
      visualTreatment: "editorial schedule marker with date lockup and reveal line",
      motion: "date marker wipes in, deadline label reveals, then masks out",
      expectedTemplateId: "chapter-card-v1",
    },
  ])(
    "previews $label immersive HyperFrame scenes with a scene-specific template",
    ({
      id,
      label,
      sourceBeat,
      visualTreatment,
      motion,
      fields,
      expectedTemplateId,
    }) => {
      const hyperframeTimeline: Timeline = {
        ...timeline,
        tracks: [
          {
            id: "track-scenes",
            kind: "hyperframe_scene",
            name: "HyperFrames",
            locked: false,
            enabled: true,
            items: [
              {
                id,
                kind: "hyperframe_scene",
                startSeconds: 0,
                durationSeconds: 1,
                label,
                source: { type: "text", text: label },
                properties: {
                  kind: "immersive_scene",
                  sourceBeat,
                  visualTreatment,
                  motion,
                  fields,
                },
              },
            ],
          },
        ],
      };

      const frame = buildTimelinePreviewFrame({
        timeline: hyperframeTimeline,
        media,
        playheadSeconds: 0.5,
      });

      expect(frame.status).toBe("ready");
      expect(frame.overlayLayers).toHaveLength(1);
      expect(requiredAt(frame.overlayLayers, 0, "overlay layer").templateId).toBe(expectedTemplateId);
    },
  );

  it("excludes disabled tracks from media and overlay preview layers", () => {
    const disabled: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          enabled: false,
        },
        {
          id: "track-captions",
          kind: "caption",
          name: "Captions",
          locked: false,
          enabled: false,
          items: [
            {
              id: "caption-1",
              kind: "caption",
              startSeconds: 1,
              durationSeconds: 2,
              label: "Caption",
              source: { type: "text", text: "Hidden" },
              properties: {},
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: disabled,
      media,
      playheadSeconds: 2,
    });

    expect(frame.layers).toEqual([]);
    expect(frame.overlayLayers).toEqual([]);
    expect(frame.status).toBe("empty");
  });

  it("reports missing audio media without manufacturing a visual layer", () => {
    const audioTimeline: Timeline = {
      ...timeline,
      tracks: [
        {
          id: "track-audio",
          kind: "audio",
          name: "Audio",
          locked: false,
          enabled: true,
          items: [
            {
              id: "music-bed",
              kind: "audio_clip",
              startSeconds: 0,
              durationSeconds: 4,
              label: "Music bed",
              source: { type: "media", mediaId: "missing-audio" },
              properties: {},
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: audioTimeline,
      media,
      playheadSeconds: 1,
    });

    expect(frame.status).toBe("missing-media");
    expect(frame.layers).toEqual([]);
    expect(frame.audioLayers).toEqual([]);
    expect(frame.issues[0]).toContain("missing-audio");
  });

  it("builds an active audio mix layer with source trimming, gain, and fade semantics", () => {
    const audioMedia: MediaAsset = {
      id: "music-media",
      relativePath: "media/music.wav",
      kind: "audio",
      durationSeconds: 10,
      width: null,
      height: null,
      fps: null,
      folderId: null,
    };
    const audioTimeline: Timeline = {
      durationSeconds: 8,
      tracks: [
        {
          id: "track-audio",
          kind: "audio",
          name: "Audio",
          locked: false,
          enabled: true,
          items: [
            {
              id: "music-bed",
              kind: "audio_clip",
              startSeconds: 1,
              durationSeconds: 4,
              label: "Music bed",
              source: { type: "media", mediaId: "music-media" },
              properties: {
                sourceIn: 2,
                sourceOut: 6,
                volumeDb: -6,
                fadeInSeconds: 1,
                fadeOutSeconds: 2,
              },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: audioTimeline,
      media: [...media, audioMedia],
      playheadSeconds: 4,
    });

    expect(frame.status).toBe("ready");
    expect(frame.layers).toEqual([]);
    expect(frame.audioLayers).toEqual([
      expect.objectContaining({
        itemId: "music-bed",
        mediaId: "music-media",
        sourceTimeSeconds: 5,
        gain: 0.251,
      }),
    ]);
    const fadedAudioLayer = requiredAt(
      buildTimelinePreviewFrame({
        timeline: audioTimeline,
        media: [...media, audioMedia],
        playheadSeconds: 1.5,
      }).audioLayers,
      0,
      "faded audio layer",
    );
    expect(fadedAudioLayer.gain).toBe(0.251);
  });

  it("interpolates canonical volume keyframes in the active audio mix", () => {
    const audioMedia: MediaAsset = {
      id: "keyframed-music",
      relativePath: "media/keyframed-music.wav",
      kind: "audio",
      durationSeconds: 4,
      width: null,
      height: null,
      fps: null,
      folderId: null,
    };
    const audioTimeline: Timeline = {
      durationSeconds: 4,
      tracks: [{
        id: "track-audio",
        kind: "audio",
        name: "Audio",
        locked: false,
        enabled: true,
        items: [{
          id: "keyframed-bed",
          kind: "audio_clip",
          startSeconds: 0,
          durationSeconds: 4,
          label: "Keyframed music",
          source: { type: "media", mediaId: "keyframed-music" },
          properties: {
            keyframes: { volumeDb: [{ atSeconds: 0, value: -20 }, { atSeconds: 4, value: 0 }] },
          },
        }],
      }],
    };

    const keyframedAudioLayer = requiredAt(
      buildTimelinePreviewFrame({
        timeline: audioTimeline,
        media: [...media, audioMedia],
        playheadSeconds: 2,
      }).audioLayers,
      0,
      "keyframed audio layer",
    );
    expect(keyframedAudioLayer.gain).toBe(0.316);
  });

  it("reports audio media placed on a visual clip as an unsupported preview source", () => {
    const audioMedia: MediaAsset = {
      id: "audio-1-media",
      relativePath: "media/dialogue.wav",
      kind: "audio",
      durationSeconds: 6,
      width: null,
      height: null,
      fps: null,
      folderId: null,
    };
    const visualTimelineWithAudio: Timeline = {
      ...timeline,
      tracks: [
        {
          ...videoTrack,
          items: [
            {
              ...openingItem,
              id: "audio-as-video",
              label: "Dialogue as picture",
              source: { type: "media", mediaId: "audio-1-media" },
            },
          ],
        },
      ],
    };

    const frame = buildTimelinePreviewFrame({
      timeline: visualTimelineWithAudio,
      media: [...media, audioMedia],
      playheadSeconds: 2,
    });

    expect(frame.status).toBe("unsupported-source");
    expect(frame.layers).toEqual([]);
    expect(frame.issues[0]).toContain("audio media");
    expect(frame.issues[0]).toContain("audio-as-video");
  });

  it("hit-tests transformed and cropped preview geometry in viewport pixel space", () => {
    const base = buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 2 }).layers[0];
    expect(base).toBeDefined();
    if (!base) return;
    const layer = {
      ...base,
      centerX: 0.5,
      centerY: 0.5,
      width: 0.5,
      height: 0.5,
      rotationDegrees: 90,
      cropLeft: 0.25,
    };
    expect(
      timelinePreviewLayerContainsPoint(layer, { x: 0.5, y: 0.5 }, { width: 1600, height: 900 }),
    ).toBe(true);
    expect(
      timelinePreviewLayerContainsPoint(layer, { x: 0.5, y: 0.2 }, { width: 1600, height: 900 }),
    ).toBe(false);
  });

  it("hit-tests position offsets in output pixels when given the render size", () => {
    const base = buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 2 }).layers[0];
    expect(base).toBeDefined();
    if (!base) return;
    // A 10%-wide layer moved 480 of 1920 output pixels right sits at x = 0.75 on any canvas size.
    const layer = { ...base, centerX: 0.5, centerY: 0.5, width: 0.1, height: 0.1, positionX: 480, positionY: -270 };
    const output = { width: 1920, height: 1080 };
    expect(timelinePreviewLayerContainsPoint(layer, { x: 0.75, y: 0.25 }, output)).toBe(true);
    expect(timelinePreviewLayerContainsPoint(layer, { x: 0.5, y: 0.5 }, output)).toBe(false);
  });

  it("selects the last rendered matching layer as the deterministic topmost hit", () => {
    const base = buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 2 }).layers[0];
    expect(base).toBeDefined();
    if (!base) return;
    const bottom = { ...base, itemId: "bottom" };
    const top = { ...base, itemId: "top", opacity: 0.7 };

    expect(
      topmostTimelinePreviewLayerAtPoint(
        [bottom, top],
        { x: 0.5, y: 0.5 },
        { width: 1600, height: 900 },
      )?.itemId,
    ).toBe("top");
    expect(
      topmostTimelinePreviewLayerAtPoint(
        [bottom, { ...top, opacity: 0 }],
        { x: 0.5, y: 0.5 },
        { width: 1600, height: 900 },
      )?.itemId,
    ).toBe("bottom");
  });
});

describe("timeline preview transitions", () => {
  // Two 4 s cuts of the 10 s clip meeting at 4 s: "left" uses source 2-6 s (4 s tail handle) and
  // "right" uses source 5-9 s (5 s head handle). A 1 s transition spans [3.5, 4.5).
  function cutClip(id: string, startSeconds: number, sourceIn: number, properties: TimelineItem["properties"] = {}): TimelineItem {
    return {
      id,
      kind: "video_clip",
      startSeconds,
      durationSeconds: 4,
      label: id,
      source: { type: "media", mediaId: "clip-1-media" },
      properties: { sourceIn, sourceOut: sourceIn + 4, ...properties },
    };
  }

  function cutTrack(kind: TransitionKind = "crossfade", durationSeconds = 1, items = [cutClip("left", 0, 2), cutClip("right", 4, 5)]): TimelineTrack {
    return {
      id: "cut-track",
      kind: "video",
      name: "Video",
      locked: false,
      enabled: true,
      items,
      transitions: [{ id: "cut", leftItemId: "left", rightItemId: "right", kind, durationSeconds }],
    };
  }

  function frameAt(playheadSeconds: number, track: TimelineTrack = cutTrack()) {
    return buildTimelinePreviewFrame({ timeline: { durationSeconds: 8, tracks: [track] }, media, playheadSeconds, fps: 24 });
  }

  function layer(frame: ReturnType<typeof frameAt>, itemId: string) {
    const found = frame.layers.find((candidate) => candidate.itemId === itemId);
    if (!found) throw new Error(`Expected layer ${itemId}`);
    return found;
  }

  it("draws both clips at the midpoint, outgoing beneath incoming, sampling their handles", () => {
    // The right clip listed first still draws on top.
    const track = cutTrack("crossfade", 1, [cutClip("right", 4, 5), cutClip("left", 0, 2)]);
    const midpoint = frameAt(4, track);
    expect(midpoint.layers.map((candidate) => candidate.itemId)).toEqual(["left", "right"]);
    expect(midpoint.issues).toEqual([]);
    expect(midpoint.transitions).toEqual([
      { transitionId: "cut", kind: "crossfade", leftItemId: "left", rightItemId: "right", startSeconds: 3.5, durationSeconds: 1, progress: 0.5, solidColor: null },
    ]);
    expect(layer(midpoint, "left")).toMatchObject({ timelineStartSeconds: 0, timelineEndSeconds: 4, sourceTimeSeconds: 6 });
    expect(layer(midpoint, "right")).toMatchObject({ timelineStartSeconds: 4, timelineEndSeconds: 8, sourceTimeSeconds: 5 });

    // Past the left clip's canonical end: sourceOut + (t - leftEnd) * speed.
    expect(layer(frameAt(4.25), "left").sourceTimeSeconds).toBe(6.25);
    // Before the right clip's canonical start: sourceIn - (rightStart - t) * speed.
    expect(layer(frameAt(3.75), "right").sourceTimeSeconds).toBe(4.75);
  });

  it("scales handle source time by clip speed", () => {
    const track = cutTrack("crossfade", 1, [
      { ...cutClip("left", 0, 0, { speed: 2 }), durationSeconds: 2 },
      { ...cutClip("right", 2, 6, { speed: 2 }), durationSeconds: 2 },
    ]);
    expect(layer(frameAt(2.25, track), "left").sourceTimeSeconds).toBe(4.5);
    expect(layer(frameAt(1.75, track), "right").sourceTimeSeconds).toBe(5.5);
  });

  it("crossfades with the incoming clip at opacity p over the outgoing clip at full opacity", () => {
    const midpoint = frameAt(4);
    expect(layer(midpoint, "left")).toMatchObject({ opacity: 1, transition: { transitionId: "cut", kind: "crossfade", role: "outgoing", progress: 0.5, opacity: 1 } });
    expect(layer(midpoint, "right")).toMatchObject({ opacity: 0.5, transition: { role: "incoming", progress: 0.5, opacity: 0.5 } });
    expect(layer(frameAt(3.75), "right").opacity).toBe(0.25);
    expect(layer(frameAt(4.25), "left").opacity).toBe(1);
  });

  it("dips through a solid with both clips transparent at the cut", () => {
    const track = cutTrack("dipToBlack");
    const cut = frameAt(4, track);
    expect(cut.layers.map(({ itemId, opacity }) => ({ itemId, opacity }))).toEqual([
      { itemId: "left", opacity: 0 },
      { itemId: "right", opacity: 0 },
    ]);
    expect(cut.transitions).toMatchObject([{ kind: "dipToBlack", progress: 0.5, solidColor: "black" }]);
    expect(frameAt(3.75, track).layers.map(({ opacity }) => opacity)).toEqual([0.5, 0]);
    expect(frameAt(4.25, track).layers.map(({ opacity }) => opacity)).toEqual([0, 0.5]);
    expect(frameAt(3.5, track).transitions).toMatchObject([{ progress: 0, solidColor: "black" }]);
    expect(frameAt(4, cutTrack("dipToWhite")).transitions).toMatchObject([{ kind: "dipToWhite", solidColor: "white" }]);
  });

  it("wipes the incoming clip in from the left", () => {
    const track = cutTrack("wipe");
    const midpoint = frameAt(4, track);
    expect(layer(midpoint, "left")).toMatchObject({ opacity: 1, transition: { role: "outgoing" } });
    expect(layer(midpoint, "left").transition?.wipeInsetRight).toBeUndefined();
    const incoming = layer(midpoint, "right");
    expect(incoming).toMatchObject({ opacity: 1, transition: { role: "incoming", wipeInsetRight: 0.5 } });
    expect(transitionClipPath(incoming.transition)).toBe("inset(0 50% 0 0)");
    expect(transitionClipPath(layer(frameAt(3.75, track), "right").transition)).toBe("inset(0 75% 0 0)");
    expect(transitionClipPath(layer(midpoint, "left").transition)).toBeUndefined();
  });

  it("draws only one clip outside the window, with exact edges", () => {
    for (const [seconds, itemId] of [[3.4, "left"], [4.6, "right"], [4.5, "right"]] as const) {
      const frame = frameAt(seconds);
      expect(frame.layers.map((candidate) => candidate.itemId)).toEqual([itemId]);
      expect(frame.layers[0]?.transition).toBeUndefined();
      expect(frame.layers[0]?.opacity).toBe(1);
      expect(frame).not.toHaveProperty("transitions");
    }
    const start = frameAt(3.5);
    expect(start.layers.map(({ itemId, opacity }) => ({ itemId, opacity }))).toEqual([
      { itemId: "left", opacity: 1 },
      { itemId: "right", opacity: 0 },
    ]);
    expect(start.transitions?.[0]?.progress).toBe(0);
    expect(layer(frameAt(4.5), "right").sourceTimeSeconds).toBe(5.5);
  });

  it("multiplies transition opacity with clip opacity, keyframes and fades, keeping effects", () => {
    const left = cutClip("left", 0, 2, {
      keyframes: { opacity: [{ atSeconds: 0, value: 1 }, { atSeconds: 4, value: 0.6 }] },
      effects: [{ effectType: "stylize.vignette", enabled: true }],
    });
    const right = cutClip("right", 4, 5, { opacity: 0.8, fadeInSeconds: 1 });
    const crossfade = cutTrack("crossfade", 1, [left, right]);
    // Keyframes hold their last value in the tail handle; the fade-in reads 0 before the canonical start.
    expect(layer(frameAt(4.25, crossfade), "left")).toMatchObject({ opacity: 0.6, effects: { vignette: true, grain: false } });
    expect(layer(frameAt(3.75, crossfade), "right").opacity).toBe(0);
    expect(layer(frameAt(4.25, crossfade), "right").opacity).toBeCloseTo(0.8 * 0.25 * 0.75, 10);
    const dip = cutTrack("dipToBlack", 1, [left, cutClip("right", 4, 5, { opacity: 0.8 })]);
    // Keyframed 0.625 at 3.75 s, halved by the dip.
    expect(layer(frameAt(3.75, dip), "left").opacity).toBeCloseTo(0.625 * 0.5, 10);
    expect(layer(frameAt(4.25, dip), "right").opacity).toBeCloseTo(0.8 * 0.5, 10);
  });

  it("clamps the window to the available handles and skips transitions that cannot draw", () => {
    // 0.2 s of media before the right clip allows 0.4 s.
    const short = cutTrack("crossfade", 1, [cutClip("left", 0, 2), cutClip("right", 4, 0.2)]);
    expect(frameAt(3.75, short).layers.map((candidate) => candidate.itemId)).toEqual(["left"]);
    expect(frameAt(3.85, short).transitions?.[0]?.durationSeconds).toBeCloseTo(0.4, 6);
    const apart = cutTrack("crossfade", 1, [cutClip("left", 0, 2), cutClip("right", 5, 5)]);
    expect(frameAt(4.25, apart).layers).toEqual([]);
  });

  it("retimes transitions inside a nested sequence by its playback speed", () => {
    const nested: Timeline = { durationSeconds: 8, tracks: [cutTrack()] };
    const frame = (playheadSeconds: number) =>
      buildTimelinePreviewFrame({
        timeline: {
          durationSeconds: 14,
          tracks: [{
            ...videoTrack,
            items: [{ ...openingItem, id: "wrapper", source: { type: "timeline", timelineId: "nested-cut" }, startSeconds: 10, durationSeconds: 4, properties: { speed: 2 } }],
          }],
        },
        timelines: [{ id: "nested-cut", timeline: nested }],
        media,
        playheadSeconds,
      });
    // The 1 s nested transition at the 12 s cut plays in 0.5 s: [11.75, 12.25).
    const midpoint = frame(12);
    expect(midpoint.layers.map(({ itemId, opacity }) => ({ itemId, opacity }))).toEqual([
      { itemId: "root:0:wrapper:left", opacity: 1 },
      { itemId: "root:0:wrapper:right", opacity: 0.5 },
    ]);
    expect(midpoint.transitions).toMatchObject([{ transitionId: "root:0:wrapper:cut", startSeconds: 11.75, durationSeconds: 0.5, progress: 0.5 }]);
    expect(frame(12.125).layers.find((candidate) => candidate.itemId.endsWith(":left"))?.sourceTimeSeconds).toBe(6.25);
    expect(frame(11.7).layers.map((candidate) => candidate.itemId)).toEqual(["root:0:wrapper:left"]);
    expect(frame(12.25).layers.map((candidate) => candidate.itemId)).toEqual(["root:0:wrapper:right"]);
  });

  it("crossfades audio pairs with equal power", () => {
    const audioMedia: MediaAsset = { id: "music", relativePath: "media/music.m4a", kind: "audio", durationSeconds: 10, width: null, height: null, fps: null, folderId: null };
    const audioClip = (item: TimelineItem): TimelineItem => ({ ...item, kind: "audio_clip", source: { type: "media", mediaId: "music" } });
    const track: TimelineTrack = { ...cutTrack(), kind: "audio", items: [audioClip(cutClip("left", 0, 2)), audioClip(cutClip("right", 4, 5))] };
    const frame = (playheadSeconds: number) =>
      buildTimelinePreviewFrame({ timeline: { durationSeconds: 8, tracks: [track] }, media: [audioMedia], playheadSeconds });
    const midpoint = frame(4);
    expect(midpoint.layers).toEqual([]);
    expect(midpoint).not.toHaveProperty("transitions");
    expect(midpoint.audioLayers.map(({ itemId, gain, sourceTimeSeconds, transition }) => ({ itemId, gain, sourceTimeSeconds, role: transition?.role }))).toEqual([
      { itemId: "left", gain: 0.707, sourceTimeSeconds: 6, role: "outgoing" },
      { itemId: "right", gain: 0.707, sourceTimeSeconds: 5, role: "incoming" },
    ]);
    const quarter = frame(3.75).audioLayers;
    expect(quarter.map(({ gain }) => gain)).toEqual([Number(Math.cos(Math.PI / 8).toFixed(3)), Number(Math.sin(Math.PI / 8).toFixed(3))]);
    expect(quarter[1]?.sourceTimeSeconds).toBe(4.75);
    expect(frame(4.5).audioLayers.map(({ itemId, gain }) => ({ itemId, gain }))).toEqual([{ itemId: "right", gain: 1 }]);
  });

  it("builds identical frames for timelines without transitions", () => {
    const withEmptyTransitions: Timeline = { ...timeline, tracks: timeline.tracks.map((track) => ({ ...track, transitions: [] })) };
    for (const playheadSeconds of [0, 1, 2, 3.99, 4, 5, 6.5, 8]) {
      const plain = buildTimelinePreviewFrame({ timeline, media, generatedAssets, playheadSeconds });
      expect(buildTimelinePreviewFrame({ timeline: withEmptyTransitions, media, generatedAssets, playheadSeconds, fps: 30 })).toEqual(plain);
      expect(plain).not.toHaveProperty("transitions");
    }
  });
});

describe("timeline preview clip speed", () => {
  const longMusic: MediaAsset = { id: "long-music", relativePath: "media/long-music.wav", kind: "audio", durationSeconds: 20, width: null, height: null, fps: null, folderId: null };

  function audioItem(id: string, startSeconds: number, sourceIn: number, speed: number, mediaId = "long-music"): TimelineItem {
    return {
      id,
      kind: "audio_clip",
      startSeconds,
      durationSeconds: 4,
      label: id,
      source: { type: "media", mediaId },
      properties: { sourceIn, sourceOut: sourceIn + 4 * speed, ...(speed === 1 ? {} : { speed }) },
    };
  }

  function audioTrack(items: TimelineItem[], transitions: TimelineTrack["transitions"] = []): TimelineTrack {
    return { id: "track-audio", kind: "audio", name: "Audio", locked: false, enabled: true, items, transitions };
  }

  function audioFrame(track: TimelineTrack, playheadSeconds: number, extraMedia: MediaAsset[] = []) {
    return buildTimelinePreviewFrame({ timeline: { durationSeconds: 8, tracks: [track] }, media: [...media, longMusic, ...extraMedia], generatedAssets, playheadSeconds, fps: 24 });
  }

  it("maps retimed audio through its speed and plays the element at that rate", () => {
    const frame = audioFrame(audioTrack([audioItem("fast", 1, 2, 2)]), 2.5);
    expect(frame.issues).toEqual([]);
    expect(frame.audioLayers).toEqual([expect.objectContaining({ itemId: "fast", sourceTimeSeconds: 5, playbackRate: 2 })]);
    expect(audioFrame(audioTrack([audioItem("plain", 1, 2, 1)]), 2.5).audioLayers).toEqual([
      expect.objectContaining({ itemId: "plain", sourceTimeSeconds: 3.5, playbackRate: 1 }),
    ]);
  });

  it("samples retimed audio crossfade handles at clip speed", () => {
    const track = audioTrack(
      [audioItem("left", 0, 1, 2), audioItem("right", 4, 5, 2)],
      [{ id: "fade", leftItemId: "left", rightItemId: "right", kind: "crossfade", durationSeconds: 1 }],
    );
    const layerOf = (playheadSeconds: number, itemId: string) => {
      const found = audioFrame(track, playheadSeconds).audioLayers.find((candidate) => candidate.itemId === itemId);
      if (!found) throw new Error(`Expected audio layer ${itemId}`);
      return found;
    };
    // Outgoing tail: sourceOut 9 + (4.25 - 4) * 2.
    expect(layerOf(4.25, "left")).toMatchObject({ sourceTimeSeconds: 9.5, playbackRate: 2, transition: expect.objectContaining({ role: "outgoing" }) });
    // Incoming head: sourceIn 5 - (4 - 3.75) * 2.
    expect(layerOf(3.75, "right")).toMatchObject({ sourceTimeSeconds: 4.5, playbackRate: 2, transition: expect.objectContaining({ role: "incoming" }) });
  });

  it("plays audio clips whose source is video or generated media", () => {
    const fromVideo = audioFrame(audioTrack([{ ...audioItem("video-sound", 0, 2, 1, "clip-1-media"), durationSeconds: 3, properties: { sourceIn: 2, sourceOut: 5 } }]), 1);
    expect(fromVideo.status).toBe("ready");
    expect(fromVideo.issues).toEqual([]);
    expect(fromVideo.audioLayers).toEqual([expect.objectContaining({ itemId: "video-sound", mediaId: "clip-1-media", relativePath: "media/clip-1.mp4", sourceTimeSeconds: 3 })]);

    const generatedSound: TimelineItem = {
      id: "generated-sound",
      kind: "audio_clip",
      startSeconds: 0,
      durationSeconds: 4,
      label: "Generated sound",
      source: { type: "generated", artifactId: "generated-shot-1" },
      properties: {},
    };
    const fromGenerated = audioFrame(audioTrack([generatedSound]), 1);
    expect(fromGenerated.issues.join(" ")).not.toContain("references");
    expect(fromGenerated.audioLayers).toEqual([expect.objectContaining({ itemId: "generated-sound", relativePath: "generated/shot-1.mp4" })]);
  });

  it("gives visual layers their clip speed as the playback rate", () => {
    const retimed: Timeline = { ...timeline, tracks: [{ ...videoTrack, items: [{ ...openingItem, properties: { ...openingItem.properties, speed: 1.5 } }] }] };
    expect(buildTimelinePreviewFrame({ timeline: retimed, media, playheadSeconds: 2 }).layers[0]).toMatchObject({ itemId: "clip-1", playbackRate: 1.5 });
    expect(buildTimelinePreviewFrame({ timeline, media, playheadSeconds: 2 }).layers[0]).toMatchObject({ itemId: "clip-1", playbackRate: 1 });
  });
});

 it("plans shader references for canonical preparation without requiring generated provider assets", () => {
   const frame = buildTimelinePreviewFrame({ media: [], playheadSeconds: 1,
     timeline: { durationSeconds: 4, tracks: [{ id: "graphics", name: "Graphics", kind: "hyperframe_scene", enabled: true, locked: false,
       items: [{ id: "shader", kind: "hyperframe_scene", startSeconds: 0, durationSeconds: 4,
         source: { type: "generated", artifactId: "orphan" }, label: "Shader", properties: { shaderBackgroundTemplateId: "shadertoy-octagrams-v1" } }],
     }] },
   });
   expect(frame).toMatchObject({ status: "ready", layers: [], overlayLayers: [], issues: [], canonicalTemplateItemIds: ["shader"] });
 });
