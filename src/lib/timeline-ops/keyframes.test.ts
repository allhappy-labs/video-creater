import { describe, expect, it } from "vitest";
import {
  audioKeyframePropertyConfigs,
  inspectorEffectParameterKeyframes,
  inspectorKeyframesByProperty,
  keyframePropertyConfigsForItem,
  motionKeyframeBounds,
  motionKeyframeDefault,
  sortedKeyframes,
  suggestedValue,
  visualKeyframePropertyConfigs,
  type KeyframePropertyConfig,
  type VisualMotionKeyframeProperty,
} from "@/lib/timeline-ops/keyframes";
import type { ProjectActionKeyframe } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

const motionProperties: VisualMotionKeyframeProperty[] = ["positionX", "positionY", "scale", "rotationDegrees"];

describe("keyframe characterization", () => {
  it("derives motion keyframe defaults and bounds", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const items: Array<TimelineItem | null> = [
      null,
      { ...video, properties: {} },
      { ...video, properties: { positionX: 12.5, positionY: "4", scale: Number.NaN, rotationDegrees: -90 } },
    ];
    expect(
      items.map((item) => motionProperties.map((property) => [property, motionKeyframeDefault(item, property)])),
    ).toMatchInlineSnapshot(`
      [
        [
          [
            "positionX",
            "0",
          ],
          [
            "positionY",
            "0",
          ],
          [
            "scale",
            "1",
          ],
          [
            "rotationDegrees",
            "0",
          ],
        ],
        [
          [
            "positionX",
            "0",
          ],
          [
            "positionY",
            "0",
          ],
          [
            "scale",
            "1",
          ],
          [
            "rotationDegrees",
            "0",
          ],
        ],
        [
          [
            "positionX",
            "12.5",
          ],
          [
            "positionY",
            "0",
          ],
          [
            "scale",
            "1",
          ],
          [
            "rotationDegrees",
            "-90",
          ],
        ],
      ]
    `);
    expect(motionProperties.map((property) => [property, motionKeyframeBounds(property)])).toMatchInlineSnapshot(`
      [
        [
          "positionX",
          {
            "max": 10000,
            "min": -10000,
            "step": 1,
          },
        ],
        [
          "positionY",
          {
            "max": 10000,
            "min": -10000,
            "step": 1,
          },
        ],
        [
          "scale",
          {
            "max": 100,
            "min": 0.01,
            "step": 0.01,
          },
        ],
        [
          "rotationDegrees",
          {
            "max": 360,
            "min": -360,
            "step": 1,
          },
        ],
      ]
    `);
  });

  it("lists keyframe property configs", () => {
    expect(visualKeyframePropertyConfigs).toMatchInlineSnapshot(`
      [
        {
          "defaultValue": 1,
          "label": "Opacity",
          "maximum": 1,
          "minimum": 0,
          "property": "opacity",
          "step": 0.05,
        },
        {
          "defaultValue": 0,
          "label": "Position X",
          "maximum": 10000,
          "minimum": -10000,
          "property": "positionX",
          "step": 1,
        },
        {
          "defaultValue": 0,
          "label": "Position Y",
          "maximum": 10000,
          "minimum": -10000,
          "property": "positionY",
          "step": 1,
        },
        {
          "defaultValue": 1,
          "label": "Scale",
          "maximum": 100,
          "minimum": 0.01,
          "property": "scale",
          "step": 0.01,
        },
        {
          "defaultValue": 0,
          "label": "Rotation",
          "maximum": 360,
          "minimum": -360,
          "property": "rotationDegrees",
          "step": 1,
        },
        {
          "defaultValue": 0,
          "label": "Crop top",
          "maximum": 0.95,
          "minimum": 0,
          "property": "cropTop",
          "step": 0.01,
        },
        {
          "defaultValue": 0,
          "label": "Crop right",
          "maximum": 0.95,
          "minimum": 0,
          "property": "cropRight",
          "step": 0.01,
        },
        {
          "defaultValue": 0,
          "label": "Crop bottom",
          "maximum": 0.95,
          "minimum": 0,
          "property": "cropBottom",
          "step": 0.01,
        },
        {
          "defaultValue": 0,
          "label": "Crop left",
          "maximum": 0.95,
          "minimum": 0,
          "property": "cropLeft",
          "step": 0.01,
        },
      ]
    `);
    expect(audioKeyframePropertyConfigs).toMatchInlineSnapshot(`
      [
        {
          "defaultValue": 0,
          "label": "Volume dB",
          "maximum": 24,
          "minimum": -60,
          "property": "volumeDb",
          "step": 0.5,
        },
      ]
    `);
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const audio = fixtureItem(project, "audio");
    expect(
      [
        { ...video, properties: { opacity: 0.5, scale: Number.NaN, cropTop: "0.2", positionX: 40 } },
        { ...audio, properties: { volumeDb: -12, opacity: 0.25 } },
        { ...audio, properties: {} },
        { ...video, kind: "caption", properties: {} },
      ].map((item) => keyframePropertyConfigsForItem(item as TimelineItem)),
    ).toMatchInlineSnapshot(`
      [
        [
          {
            "defaultValue": 0.5,
            "label": "Opacity",
            "maximum": 1,
            "minimum": 0,
            "property": "opacity",
            "step": 0.05,
          },
          {
            "defaultValue": 40,
            "label": "Position X",
            "maximum": 10000,
            "minimum": -10000,
            "property": "positionX",
            "step": 1,
          },
          {
            "defaultValue": 0,
            "label": "Position Y",
            "maximum": 10000,
            "minimum": -10000,
            "property": "positionY",
            "step": 1,
          },
          {
            "defaultValue": 1,
            "label": "Scale",
            "maximum": 100,
            "minimum": 0.01,
            "property": "scale",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Rotation",
            "maximum": 360,
            "minimum": -360,
            "property": "rotationDegrees",
            "step": 1,
          },
          {
            "defaultValue": 0,
            "label": "Crop top",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropTop",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Crop right",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropRight",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Crop bottom",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropBottom",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Crop left",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropLeft",
            "step": 0.01,
          },
        ],
        [
          {
            "defaultValue": -12,
            "label": "Volume dB",
            "maximum": 24,
            "minimum": -60,
            "property": "volumeDb",
            "step": 0.5,
          },
        ],
        [
          {
            "defaultValue": 0,
            "label": "Volume dB",
            "maximum": 24,
            "minimum": -60,
            "property": "volumeDb",
            "step": 0.5,
          },
        ],
        [
          {
            "defaultValue": 1,
            "label": "Opacity",
            "maximum": 1,
            "minimum": 0,
            "property": "opacity",
            "step": 0.05,
          },
          {
            "defaultValue": 0,
            "label": "Position X",
            "maximum": 10000,
            "minimum": -10000,
            "property": "positionX",
            "step": 1,
          },
          {
            "defaultValue": 0,
            "label": "Position Y",
            "maximum": 10000,
            "minimum": -10000,
            "property": "positionY",
            "step": 1,
          },
          {
            "defaultValue": 1,
            "label": "Scale",
            "maximum": 100,
            "minimum": 0.01,
            "property": "scale",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Rotation",
            "maximum": 360,
            "minimum": -360,
            "property": "rotationDegrees",
            "step": 1,
          },
          {
            "defaultValue": 0,
            "label": "Crop top",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropTop",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Crop right",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropRight",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Crop bottom",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropBottom",
            "step": 0.01,
          },
          {
            "defaultValue": 0,
            "label": "Crop left",
            "maximum": 0.95,
            "minimum": 0,
            "property": "cropLeft",
            "step": 0.01,
          },
        ],
      ]
    `);
  });

  it("reads inspector keyframes by property", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const items: TimelineItem[] = [
      { ...video, id: "no-keyframes", properties: {} },
      { ...video, id: "keyframes-array", properties: { keyframes: [] } },
      {
        ...video,
        id: "mixed",
        properties: {
          keyframes: {
            opacity: [
              { atSeconds: 3, value: 1, easing: "smooth" },
              { atSeconds: 1, value: 0, easing: "bounce" },
              { atSeconds: -1, value: 2, easing: "hold" },
              { atSeconds: Number.NaN, value: 1 },
              { atSeconds: 2, value: "1" },
              null,
            ],
            volumeDb: [{ atSeconds: 0, value: -6, easing: null }],
            scaleX: [{ atSeconds: 0, value: 2 }],
            scale: [],
            cropLeft: "0.1",
            rotationDegrees: [{ atSeconds: 0.5, value: 45, easing: "easeInOut" }],
          },
        },
      },
    ];
    expect(items.map((item) => [item.id, inspectorKeyframesByProperty(item)])).toMatchInlineSnapshot(`
      [
        [
          "no-keyframes",
          {},
        ],
        [
          "keyframes-array",
          {},
        ],
        [
          "mixed",
          {
            "opacity": [
              {
                "atSeconds": 3,
                "easing": "smooth",
                "value": 1,
              },
              {
                "atSeconds": 1,
                "value": 0,
              },
              {
                "atSeconds": -1,
                "easing": "hold",
                "value": 2,
              },
            ],
            "rotationDegrees": [
              {
                "atSeconds": 0.5,
                "easing": "easeInOut",
                "value": 45,
              },
            ],
            "scale": [],
            "volumeDb": [
              {
                "atSeconds": 0,
                "value": -6,
              },
            ],
          },
        ],
      ]
    `);
  });

  it("reads effect parameter keyframes", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const item: TimelineItem = {
      ...video,
      properties: {
        effectParameterKeyframes: {
          "blur-1": {
            radius: [
              { atSeconds: 2, value: 8, easing: "easeIn" },
              { atSeconds: 0, value: 0, easing: "wobble" },
              { atSeconds: Number.NaN, value: 4 },
              { atSeconds: 1, value: "4" },
              [],
            ],
            amount: "0.5",
          },
          "glow-1": [],
        },
      },
    };
    expect(
      [
        ["blur-1", "radius"],
        ["blur-1", "amount"],
        ["blur-1", "missing"],
        ["glow-1", "radius"],
        ["missing", "radius"],
      ].map(([instance, key]) => [instance, key, inspectorEffectParameterKeyframes(item, instance!, key!)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "blur-1",
          "radius",
          [
            {
              "atSeconds": 2,
              "easing": "easeIn",
              "value": 8,
            },
            {
              "atSeconds": 0,
              "easing": "linear",
              "value": 0,
            },
            {
              "atSeconds": NaN,
              "easing": "linear",
              "value": 4,
            },
          ],
        ],
        [
          "blur-1",
          "amount",
          [],
        ],
        [
          "blur-1",
          "missing",
          [],
        ],
        [
          "glow-1",
          "radius",
          [],
        ],
        [
          "missing",
          "radius",
          [],
        ],
      ]
    `);
    expect(inspectorEffectParameterKeyframes({ ...video, properties: {} }, "blur-1", "radius")).toMatchInlineSnapshot(`[]`);
    expect(
      inspectorEffectParameterKeyframes({ ...video, properties: { effectParameterKeyframes: [] } }, "blur-1", "radius"),
    ).toMatchInlineSnapshot(`[]`);
  });

  it("sorts keyframes without mutating the input", () => {
    const unsorted: ProjectActionKeyframe[] = [
      { atSeconds: 3, value: 3 },
      { atSeconds: 1, value: 1 },
      { atSeconds: 2, value: 2 },
      { atSeconds: 1, value: 10 },
    ];
    const snapshot = structuredClone(unsorted);
    expect(sortedKeyframes(unsorted)).toMatchInlineSnapshot(`
      [
        {
          "atSeconds": 1,
          "value": 1,
        },
        {
          "atSeconds": 1,
          "value": 10,
        },
        {
          "atSeconds": 2,
          "value": 2,
        },
        {
          "atSeconds": 3,
          "value": 3,
        },
      ]
    `);
    expect(unsorted).toEqual(snapshot);
    expect([sortedKeyframes([{ atSeconds: 2, value: 5 }]), sortedKeyframes([]), sortedKeyframes(undefined)]).toMatchInlineSnapshot(`
      [
        [
          {
            "atSeconds": 2,
            "value": 5,
          },
        ],
        [],
        [],
      ]
    `);
  });

  it("suggests values from surrounding keyframes", () => {
    const config: KeyframePropertyConfig = {
      property: "opacity",
      label: "Opacity",
      minimum: 0,
      maximum: 1,
      step: 0.05,
      defaultValue: 0.75,
    };
    const probes = [0, 1, 1.5, 2, 2.5, 3, 4];
    const lists: Array<[string, ProjectActionKeyframe[]]> = [
      ["empty", []],
      ["single", [{ atSeconds: 2, value: 0.4 }]],
      ["sorted", [
        { atSeconds: 1, value: 0 },
        { atSeconds: 2, value: 1, easing: "hold" },
        { atSeconds: 3, value: 0.5 },
      ]],
      ["unsorted", [
        { atSeconds: 3, value: 0.5 },
        { atSeconds: 1, value: 0 },
        { atSeconds: 2, value: 1, easing: "hold" },
      ]],
    ];
    expect(
      lists.map(([name, keyframes]) => [
        name,
        probes.map((atSeconds) => [atSeconds, suggestedValue(config, keyframes, atSeconds)]),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "empty",
          [
            [
              0,
              0.75,
            ],
            [
              1,
              0.75,
            ],
            [
              1.5,
              0.75,
            ],
            [
              2,
              0.75,
            ],
            [
              2.5,
              0.75,
            ],
            [
              3,
              0.75,
            ],
            [
              4,
              0.75,
            ],
          ],
        ],
        [
          "single",
          [
            [
              0,
              0.4,
            ],
            [
              1,
              0.4,
            ],
            [
              1.5,
              0.4,
            ],
            [
              2,
              0.4,
            ],
            [
              2.5,
              0.4,
            ],
            [
              3,
              0.4,
            ],
            [
              4,
              0.4,
            ],
          ],
        ],
        [
          "sorted",
          [
            [
              0,
              0,
            ],
            [
              1,
              0,
            ],
            [
              1.5,
              0.5,
            ],
            [
              2,
              1,
            ],
            [
              2.5,
              1,
            ],
            [
              3,
              0.5,
            ],
            [
              4,
              0.5,
            ],
          ],
        ],
        [
          "unsorted",
          [
            [
              0,
              0.5,
            ],
            [
              1,
              0,
            ],
            [
              1.5,
              0.125,
            ],
            [
              2,
              1,
            ],
            [
              2.5,
              1,
            ],
            [
              3,
              0.5,
            ],
            [
              4,
              1,
            ],
          ],
        ],
      ]
    `);
  });
});
