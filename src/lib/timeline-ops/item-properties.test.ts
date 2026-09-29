import { describe, expect, it } from "vitest";
import {
  audioDenoiseAmount,
  audioDenoisePreparationStatus,
  audioFadeInSeconds,
  audioFadeOutSeconds,
  clampPlayheadSecondsForTimeline,
  clampTimelinePlayhead,
  colorGradeNumberProperty,
  formatPercent,
  generatedWorkflowStatusTitle,
  hasEffect,
  isGeneratedTimelineItem,
  isVisualOpacityClip,
  isVisualOpacityItem,
  itemMetadata,
  itemNeedsCanonicalViewerPreparation,
  nextCatalogEffectInstanceId,
  numberProperty,
  projectActionEffectsForItem,
  sourceEffectDrafts,
  stringProperty,
  stringPropertyOrFallback,
  timelineItemSourceMediaId,
  transformBooleanProperty,
  transformNumberProperty,
  visualBlendModeProperty,
  visualClipOpacity,
} from "@/lib/timeline-ops/item-properties";
import type { ProjectActionEffect, VisualEffectDescriptor } from "@/lib/project";
import type { TimelineItem, TimelineItemKind } from "@/lib/timeline";
import { fixtureItem, fixtureMedia, fixtureProject } from "@/test-utils/editor-fixtures";

const itemKinds: TimelineItemKind[] = [
  "video_clip",
  "image_clip",
  "lottie_clip",
  "generated_clip",
  "hyperframe_scene",
  "overlay",
  "caption",
  "audio_clip",
];

function probeItems(): TimelineItem[] {
  const project = fixtureProject();
  const projectItems = project.timeline.tracks.flatMap((track) => track.items);
  const video = fixtureItem(project, "video");
  const audio = fixtureItem(project, "audio");
  return [
    ...projectItems,
    { ...video, id: "no-properties", properties: {} },
    {
      ...video,
      id: "stringy-properties",
      properties: {
        sourceIn: "0.5",
        sourceOut: "",
        opacity: Number.NaN,
        volumeDb: Number.POSITIVE_INFINITY,
        reason: "   ",
        generatedAssetId: "",
        blendMode: "multiply",
      },
    },
    {
      ...video,
      id: "generated-source",
      kind: "overlay",
      source: { type: "generated", artifactId: "artifact-7" },
      properties: { opacity: 0.5, templateId: "template-lower-third" },
    },
    {
      ...video,
      id: "generated-source-no-template",
      kind: "hyperframe_scene",
      source: { type: "generated", artifactId: "artifact-8" },
      properties: { opacity: 1 },
    },
    {
      ...audio,
      id: "audio-automation",
      properties: {
        fadeInSeconds: 9,
        fadeOutSeconds: -1,
        volumeDb: -3.456,
        generatedOutputMediaId: "media-generated-audio",
      },
    },
    {
      ...audio,
      id: "audio-zero-duration",
      durationSeconds: 0,
      properties: { fadeInSeconds: 0.5, fadeOutSeconds: 0.5, volumeDb: 2 },
    },
    {
      ...video,
      id: "image-with-opacity",
      kind: "image_clip",
      properties: { opacity: 0.25, blendMode: "not-a-mode" },
    },
    {
      ...video,
      id: "text-item",
      kind: "caption",
      source: { type: "text", text: "hello" },
      properties: { reason: "caption reason", opacity: 0.4 },
    },
  ];
}

const propertyKeys = [
  "sourceIn",
  "sourceOut",
  "opacity",
  "volumeDb",
  "fadeInSeconds",
  "reason",
  "generatedAssetId",
  "transcriptId",
  "missing",
];

describe("timeline item property characterization", () => {
  it("reads number properties identically", () => {
    const items = probeItems();
    const expected = items.map((item) => propertyKeys.map((key) => numberProperty(item, key)));
    expect(expected).toMatchInlineSnapshot(`
      [
        [
          0,
          4,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          0,
          4,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          0.65,
          2,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          2.15,
          3.35,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          0,
          4,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          0.5,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          1,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          -3.456,
          9,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          2,
          0.5,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          0.25,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          0.4,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
      ]
    `);
  });

  it("reads string properties", () => {
    const items = probeItems();
    const expected = items.map((item) => propertyKeys.map((key) => stringProperty(item, key)));
    expect(expected).toMatchInlineSnapshot(`
      [
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          "bundled local restoration demonstration",
          "sample-generated-shot",
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          "transcript-media-1",
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          "transcript-media-1",
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          "0.5",
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        [
          null,
          null,
          null,
          null,
          null,
          "caption reason",
          null,
          null,
          null,
        ],
      ]
    `);
    expect([
      ...items.map((item) => propertyKeys.map((key) => stringPropertyOrFallback(item, key))),
      propertyKeys.map((key) => stringPropertyOrFallback(null, key)),
      propertyKeys.map((key) => stringPropertyOrFallback(null, key, "fallback")),
      propertyKeys.map((key) => stringPropertyOrFallback(items[0] ?? null, key, "fallback")),
    ]).toMatchInlineSnapshot(`
      [
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "bundled local restoration demonstration",
          "sample-generated-shot",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "transcript-media-1",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "transcript-media-1",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "0.5",
          "",
          "",
          "",
          "",
          "   ",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "caption reason",
          "",
          "",
          "",
        ],
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
        ],
        [
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
        ],
        [
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
          "fallback",
        ],
      ]
    `);
  });

  it("resolves source media ids identically", () => {
    const items = probeItems();
    const expected = items.map((item) => timelineItemSourceMediaId(item));
    expect(expected).toMatchInlineSnapshot(`
      [
        "media-1",
        "sample-generated-output",
        null,
        null,
        "media-voiceover",
        "media-1",
        "media-1",
        "artifact-7",
        "artifact-8",
        "media-voiceover",
        "media-voiceover",
        "media-1",
        null,
      ]
    `);
  });

  it("clamps playheads", () => {
    const pairs: Array<[number, number]> = [
      [0, 10],
      [-1, 10],
      [4.12345, 10],
      [12, 10],
      [5, -2],
      [Number.NaN, 10],
      [5, Number.NaN],
      [5, Number.POSITIVE_INFINITY],
      [Number.POSITIVE_INFINITY, 10],
    ];
    expect(pairs.map(([seconds, duration]) => clampTimelinePlayhead(seconds, duration))).toMatchInlineSnapshot(`
      [
        0,
        0,
        4.123,
        10,
        0,
        0,
        0,
        0,
        0,
      ]
    `);
    expect(
      pairs.map(([seconds, duration]) => clampPlayheadSecondsForTimeline(seconds, duration)),
    ).toMatchInlineSnapshot(`
      [
        0,
        0,
        4.123,
        10,
        -2,
        0,
        NaN,
        5,
        0,
      ]
    `);
  });

  it("reads transform, color grade, blend, and opacity properties", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const items: TimelineItem[] = [
      video,
      { ...video, properties: { transform: { positionX: 12.5, scale: "2", flipHorizontal: true, flipVertical: "true" } } },
      { ...video, properties: { transform: [1, 2], colorGrade: [1] } },
      { ...video, properties: { transform: null, colorGrade: { exposure: 0.4, contrast: Number.NaN } } },
      { ...video, properties: { blendMode: "screen" } },
      { ...video, properties: { blendMode: "normal" } },
      { ...video, properties: { blendMode: "   " } },
    ];
    expect(
      items.map((item) => [
        transformNumberProperty(item, "positionX"),
        transformNumberProperty(item, "scale"),
        transformBooleanProperty(item, "flipHorizontal"),
        transformBooleanProperty(item, "flipVertical"),
        colorGradeNumberProperty(item, "exposure"),
        colorGradeNumberProperty(item, "contrast"),
        visualBlendModeProperty(item),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          null,
          null,
          false,
          false,
          null,
          null,
          "over",
        ],
        [
          12.5,
          null,
          true,
          false,
          null,
          null,
          "over",
        ],
        [
          null,
          null,
          false,
          false,
          null,
          null,
          "over",
        ],
        [
          null,
          null,
          false,
          false,
          0.4,
          null,
          "over",
        ],
        [
          null,
          null,
          false,
          false,
          null,
          null,
          "screen",
        ],
        [
          null,
          null,
          false,
          false,
          null,
          null,
          "over",
        ],
        [
          null,
          null,
          false,
          false,
          null,
          null,
          "over",
        ],
      ]
    `);

    const kindItems = itemKinds.map((kind) => ({ ...video, kind, properties: { opacity: 0.5 } }));
    expect(
      [...kindItems, ...probeItems()].map((item) => [
        item.kind,
        isVisualOpacityItem(item),
        isVisualOpacityClip({ kind: item.kind } as Parameters<typeof isVisualOpacityClip>[0]),
        visualClipOpacity(item),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "video_clip",
          true,
          true,
          0.5,
        ],
        [
          "image_clip",
          true,
          false,
          null,
        ],
        [
          "lottie_clip",
          true,
          false,
          null,
        ],
        [
          "generated_clip",
          true,
          false,
          null,
        ],
        [
          "hyperframe_scene",
          true,
          true,
          0.5,
        ],
        [
          "overlay",
          true,
          true,
          0.5,
        ],
        [
          "caption",
          false,
          false,
          null,
        ],
        [
          "audio_clip",
          false,
          false,
          null,
        ],
        [
          "video_clip",
          true,
          true,
          null,
        ],
        [
          "video_clip",
          true,
          true,
          null,
        ],
        [
          "caption",
          false,
          false,
          null,
        ],
        [
          "caption",
          false,
          false,
          null,
        ],
        [
          "audio_clip",
          false,
          false,
          null,
        ],
        [
          "video_clip",
          true,
          true,
          null,
        ],
        [
          "video_clip",
          true,
          true,
          null,
        ],
        [
          "overlay",
          true,
          true,
          0.5,
        ],
        [
          "hyperframe_scene",
          true,
          true,
          null,
        ],
        [
          "audio_clip",
          false,
          false,
          null,
        ],
        [
          "audio_clip",
          false,
          false,
          null,
        ],
        [
          "image_clip",
          true,
          false,
          null,
        ],
        [
          "caption",
          false,
          false,
          null,
        ],
      ]
    `);
    expect([
      isVisualOpacityItem(null),
      isVisualOpacityClip(null),
      isVisualOpacityClip(undefined),
    ]).toMatchInlineSnapshot(`
      [
        false,
        false,
        false,
      ]
    `);
  });

  it("reads effects and denoise state", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const audio = fixtureItem(project, "audio");
    const effectItems: TimelineItem[] = [
      video,
      {
        ...video,
        id: "two-effects",
        properties: {
          effects: [
            { effectInstanceId: "fx-blur-1", effectType: "blur.gaussian", enabled: true, params: { radius: 4 } },
            { effectType: "color.curves", enabled: false, params: { masterCurve: [[0, 0], [0.5, 0.62], [1, 1]] } },
          ],
        },
      },
      {
        ...video,
        id: "mixed-effects",
        properties: {
          effects: [
            null,
            [],
            "blur",
            { effectType: "blur.gaussian", enabled: "yes", params: {} },
            { effectType: "blur.gaussian", enabled: true, params: [] },
            { effectType: "blur.gaussian", enabled: true, params: { radius: 2 }, effectInstanceId: "   " },
            { effectType: "blur.gaussian", enabled: false, params: { radius: 3 } },
            { effectType: "color hue/curves", enabled: true, params: { targets: [{ targetHue: 120, satScale: "2" }] } },
            { effectType: "lut.apply", enabled: true, params: { lutPath: "luts/film.cube" } },
          ],
        },
      },
      { ...video, id: "effects-object", properties: { effects: { effectType: "blur.gaussian" } } },
      {
        ...audio,
        id: "audio-denoise",
        properties: {
          effects: [{ effectType: "audio.denoise", enabled: true, params: { amount: 0.25 } }],
        },
      },
      {
        ...audio,
        id: "audio-denoise-disabled",
        properties: {
          effects: [{ effectType: "audio.denoise", enabled: false, params: { amount: 0.25 } }],
          audioDenoisePreparation: { status: "ready" },
        },
      },
      {
        ...audio,
        id: "audio-denoise-bad-amount",
        properties: {
          effects: [{ effectType: "audio.denoise", params: { amount: Number.NaN } }],
          audioDenoisePreparation: { status: 3 },
        },
      },
      {
        ...audio,
        id: "audio-denoise-array-prep",
        properties: {
          effects: [{ effectType: "audio.denoise", enabled: true, params: [] }],
          audioDenoisePreparation: [],
        },
      },
    ];

    const expected = effectItems.map((item) => projectActionEffectsForItem(item));
    expect(expected).toMatchInlineSnapshot(`
      [
        [],
        [
          {
            "effectInstanceId": "fx-blur-1",
            "effectType": "blur.gaussian",
            "enabled": true,
            "params": {
              "radius": 4,
            },
          },
          {
            "effectInstanceId": "legacy:color.curves:1",
            "effectType": "color.curves",
            "enabled": false,
            "params": {
              "masterCurve": [
                [
                  0,
                  0,
                ],
                [
                  0.5,
                  0.62,
                ],
                [
                  1,
                  1,
                ],
              ],
            },
          },
        ],
        [
          {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "effectType": "blur.gaussian",
            "enabled": true,
            "params": {
              "radius": 2,
            },
          },
          {
            "effectInstanceId": "legacy:blur.gaussian:2",
            "effectType": "blur.gaussian",
            "enabled": false,
            "params": {
              "radius": 3,
            },
          },
          {
            "effectInstanceId": "legacy:color-hue-curves:1",
            "effectType": "color hue/curves",
            "enabled": true,
            "params": {
              "targets": [
                {
                  "satScale": "2",
                  "targetHue": 120,
                },
              ],
            },
          },
          {
            "effectInstanceId": "legacy:lut.apply:1",
            "effectType": "lut.apply",
            "enabled": true,
            "params": {
              "lutPath": "luts/film.cube",
            },
          },
        ],
        [],
        [
          {
            "effectInstanceId": "legacy:audio.denoise:1",
            "effectType": "audio.denoise",
            "enabled": true,
            "params": {
              "amount": 0.25,
            },
          },
        ],
        [
          {
            "effectInstanceId": "legacy:audio.denoise:1",
            "effectType": "audio.denoise",
            "enabled": false,
            "params": {
              "amount": 0.25,
            },
          },
        ],
        [],
        [],
      ]
    `);

    expect(
      [...effectItems.map((item) => item as TimelineItem | null), null].map((item) => [
        item?.id ?? null,
        item ? hasEffect(item, "blur.gaussian") : null,
        item ? hasEffect(item, "audio.denoise") : null,
        audioDenoiseAmount(item),
        audioDenoisePreparationStatus(item),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "item-1",
          false,
          false,
          0.6,
          null,
        ],
        [
          "two-effects",
          true,
          false,
          0.6,
          null,
        ],
        [
          "mixed-effects",
          true,
          false,
          0.6,
          null,
        ],
        [
          "effects-object",
          false,
          false,
          0.6,
          null,
        ],
        [
          "audio-denoise",
          false,
          true,
          0.25,
          "queued",
        ],
        [
          "audio-denoise-disabled",
          false,
          true,
          0.6,
          "ready",
        ],
        [
          "audio-denoise-bad-amount",
          false,
          true,
          0.6,
          null,
        ],
        [
          "audio-denoise-array-prep",
          false,
          true,
          0.6,
          "queued",
        ],
        [
          null,
          null,
          null,
          0.6,
          null,
        ],
      ]
    `);

    const catalog: VisualEffectDescriptor[] = [
      {
        id: "blur.gaussian",
        displayName: "Gaussian blur",
        category: "blur",
        params: [{ key: "radius", label: "Radius", min: 0, max: 100, defaultValue: 8, unit: "px" }],
        colorEffect: false,
      },
      {
        id: "color.curves",
        displayName: "Curves",
        category: "color",
        params: [],
        colorEffect: true,
      },
      {
        id: "color.hueCurves",
        displayName: "Hue curves",
        category: "color",
        params: [{ key: "strength", label: "Strength", min: 0, max: 1, defaultValue: 1, unit: "" }],
        colorEffect: true,
      },
      {
        id: "lut.apply",
        displayName: "LUT",
        category: "color",
        params: [],
        resourceKey: "lutPath",
        colorEffect: true,
      },
    ];
    const hueCurvesItem: TimelineItem = {
      ...video,
      id: "hue-curves",
      properties: {
        effects: [
          {
            effectInstanceId: "fx-hue",
            effectType: "color.hueCurves",
            enabled: true,
            params: { strength: 0.5, targets: [{ targetHue: 120, hueShift: "4", lumShift: -0.1 }] },
          },
          { effectType: "lut.apply", enabled: true, params: { lutPath: 42 } },
        ],
      },
    };
    expect(
      [...effectItems, hueCurvesItem, null].map((item) => sourceEffectDrafts(item, catalog)),
    ).toMatchInlineSnapshot(`
      [
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "fx-blur-1",
            "enabled": true,
            "params": {
              "radius": "4",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.62",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": true,
            "params": {
              "radius": "2",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": true,
            "params": {},
            "resource": "luts/film.cube",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "fx-hue",
            "enabled": true,
            "params": {
              "hueShift": "0",
              "lumShift": "-0.1",
              "satScale": "1",
              "strength": "0.5",
              "targetHue": "120",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": true,
            "params": {},
            "resource": "",
          },
        },
        {
          "blur.gaussian": {
            "effectInstanceId": "legacy:blur.gaussian:1",
            "enabled": false,
            "params": {
              "radius": "8",
            },
            "resource": "",
          },
          "color.curves": {
            "effectInstanceId": "legacy:color.curves:1",
            "enabled": false,
            "params": {
              "curveMidpoint": "0.5",
            },
            "resource": "",
          },
          "color.hueCurves": {
            "effectInstanceId": "legacy:color.hueCurves:1",
            "enabled": false,
            "params": {
              "hueShift": "0",
              "lumShift": "0",
              "satScale": "1",
              "strength": "1",
              "targetHue": "0",
            },
            "resource": "",
          },
          "lut.apply": {
            "effectInstanceId": "legacy:lut.apply:1",
            "enabled": false,
            "params": {},
            "resource": "",
          },
        },
      ]
    `);
  });

  it("allocates catalog effect instance ids", () => {
    const effect = (effectType: string, effectInstanceId: string): ProjectActionEffect => ({
      effectInstanceId,
      effectType,
      enabled: true,
      params: {},
    });
    expect([
      nextCatalogEffectInstanceId("blur.gaussian", []),
      nextCatalogEffectInstanceId("blur.gaussian", [effect("blur.gaussian", "fx-1")]),
      nextCatalogEffectInstanceId("blur.gaussian", [
        effect("blur.gaussian", "legacy:blur.gaussian:2"),
        effect("color.curves", "legacy:blur.gaussian:3"),
      ]),
      nextCatalogEffectInstanceId(" color hue/curves ", [effect("color.curves", "legacy:color.curves:1")]),
    ]).toMatchInlineSnapshot(`
      [
        "legacy:blur.gaussian:1",
        "legacy:blur.gaussian:2",
        "legacy:blur.gaussian:4",
        "legacy:color-hue-curves:1",
      ]
    `);
  });

  it("detects canonical viewer preparation needs", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    project.media.push({ ...fixtureMedia(project, "video"), id: "media-lottie", kind: "lottie" });
    const items: TimelineItem[] = [
      ...probeItems(),
      { ...video, source: { type: "media", mediaId: "media-lottie" }, properties: {} },
      { ...video, source: { type: "generated", artifactId: "media-lottie" }, properties: {} },
      { ...video, properties: { blendMode: "normal" } },
      { ...video, properties: { blendMode: "over" } },
      { ...video, properties: { blendMode: "screen" } },
      { ...video, properties: { blendMode: 3 } },
      { ...video, properties: { effects: [{ enabled: false }, null, []] } },
      { ...video, properties: { effects: [{ effectType: "blur" }] } },
      { ...video, properties: { colorGrade: {} } },
      { ...video, properties: { colorGrade: [] } },
      { ...video, properties: { colorGrade: { exposure: 0 } } },
    ];
    expect(items.map((item) => itemNeedsCanonicalViewerPreparation(project, item))).toMatchInlineSnapshot(`
      [
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        false,
        false,
        false,
        false,
        true,
        false,
        true,
        true,
        false,
        false,
        true,
        false,
        false,
        true,
        false,
        false,
        true,
      ]
    `);
  });

  it("describes timeline item metadata and generated state", () => {
    const items = probeItems();
    expect(
      items.map((item) => [
        item.id,
        audioFadeInSeconds(item),
        audioFadeOutSeconds(item),
        isGeneratedTimelineItem(item),
        itemMetadata(item),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "item-1",
          null,
          null,
          false,
          {
            "range": "0.00s-4.00s",
            "reason": null,
          },
        ],
        [
          "sample-generated-clip",
          null,
          null,
          true,
          {
            "range": "0.00s-4.00s",
            "reason": "bundled local restoration demonstration",
          },
        ],
        [
          "caption-1",
          null,
          null,
          false,
          {
            "range": "0.65s-2.00s",
            "reason": null,
          },
        ],
        [
          "caption-2",
          null,
          null,
          false,
          {
            "range": "2.15s-3.35s",
            "reason": null,
          },
        ],
        [
          "music-bed",
          null,
          0.75,
          false,
          {
            "range": "0.00s-4.00s",
            "reason": "fade out 0.75s",
          },
        ],
        [
          "no-properties",
          null,
          null,
          false,
          null,
        ],
        [
          "stringy-properties",
          null,
          null,
          false,
          null,
        ],
        [
          "generated-source",
          null,
          null,
          true,
          {
            "range": "generated",
            "reason": "opacity 50%",
          },
        ],
        [
          "generated-source-no-template",
          null,
          null,
          true,
          {
            "range": "generated",
            "reason": "artifact-8",
          },
        ],
        [
          "audio-automation",
          4,
          null,
          true,
          {
            "range": "audio clip",
            "reason": "fade in 4.00s, volume -3.46dB",
          },
        ],
        [
          "audio-zero-duration",
          null,
          null,
          false,
          {
            "range": "audio clip",
            "reason": "volume +2.00dB",
          },
        ],
        [
          "image-with-opacity",
          null,
          null,
          false,
          null,
        ],
        [
          "text-item",
          null,
          null,
          false,
          {
            "range": "caption",
            "reason": "caption reason",
          },
        ],
      ]
    `);
    const percentValues = [0, 0.004, 0.005, 0.125, 0.5, 1, 1.5, -0.25, Number.NaN];
    expect(percentValues.map((value) => formatPercent(value))).toMatchInlineSnapshot(`
      [
        "0%",
        "0%",
        "1%",
        "13%",
        "50%",
        "100%",
        "150%",
        "-25%",
        "NaN%",
      ]
    `);
    expect(
      generatedWorkflowStatusTitle({
        status: "running",
        updatedAt: "2026-09-13T00:00:00Z",
        workflowType: null,
        taskQueue: null,
      }),
    ).toMatchInlineSnapshot(`"Workflow running, updated 2026-09-13T00:00:00Z"`);
  });
});
