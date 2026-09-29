import { describe, expect, it } from "vitest";
import {
  canonicalCanvasCrop,
  canonicalCanvasTransform,
  clampCanvasValue,
  croppedCanvasEdges,
  findTimelineItem,
  isCanvasCropInteraction,
  minimumCanvasItemSize,
  minimumVisibleCropFraction,
  motionTemplatePreviewLaneOccupancy,
  normalizeRotationDegrees,
  previewMotionStyle,
  previewOutputSize,
  resizedCanvasTransform,
  roundedCanvasValue,
  type CanvasCropEdge,
  type CanvasCropInteraction,
  type CanvasInteraction,
  type CanvasResizeCorner,
  type CanvasTransformInteraction,
} from "@/lib/preview/canvas-geometry";
import type { VideoProject } from "@/lib/project";
import { buildTimelinePreviewFrame, type TimelinePreviewLayer } from "@/lib/timeline-preview";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { requiredValue } from "@/test-utils/required";

function frameAt(project: VideoProject, playheadSeconds: number) {
  return buildTimelinePreviewFrame({
    timeline: project.timeline,
    media: project.media,
    generatedAssets: project.generatedAssets,
    playheadSeconds,
  });
}

function transformedProject(): VideoProject {
  const project = fixtureProject();
  const videoTrack = requiredValue(
    project.timeline.tracks.find((track) => track.kind === "video"),
    "video track",
  );
  const item = requiredValue(videoTrack.items[0], "first video item");
  item.properties = {
    ...item.properties,
    rotationDegrees: 30,
    cropTop: 0.1,
    cropLeft: 0.2,
    opacity: 0,
    blendMode: "add",
    transform: { centerX: 0.25, centerY: 0.75, width: 0.5, height: 0.4, flipHorizontal: true },
    keyframes: {
      positionX: [
        { atSeconds: 0, value: 0, easing: "linear" },
        { atSeconds: 2, value: 100, easing: "linear" },
      ],
      scale: [
        { atSeconds: 0, value: 1, easing: "linear" },
        { atSeconds: 2, value: 2, easing: "linear" },
      ],
      rotationDegrees: [
        { atSeconds: 0, value: -90, easing: "linear" },
        { atSeconds: 2, value: 90, easing: "linear" },
      ],
    },
  };
  return project;
}

function baseLayer(): TimelinePreviewLayer {
  return requiredValue(frameAt(fixtureProject(), 1).layers[0], "preview layer at 1s");
}

const output = { width: 1920, height: 1080 };

const initialTransform = { centerX: 0.5, centerY: 0.5, width: 0.5, height: 0.5 };

function transformInteraction(mode: CanvasTransformInteraction["mode"]): CanvasTransformInteraction {
  return {
    itemId: "item-1",
    pointerId: 1,
    mode,
    pointerStartX: 0,
    pointerStartY: 0,
    viewportWidth: 1000,
    viewportHeight: 500,
    initial: initialTransform,
    current: initialTransform,
    moved: false,
  };
}

function cropInteraction(
  mode: CanvasCropEdge,
  initial: CanvasCropInteraction["initial"],
): CanvasCropInteraction {
  return {
    itemId: "item-1",
    pointerId: 1,
    mode,
    pointerStartX: 0,
    pointerStartY: 0,
    viewportWidth: 1000,
    viewportHeight: 500,
    initial,
    current: initial,
    moved: false,
  };
}

const corners: CanvasResizeCorner[] = ["topLeft", "topRight", "bottomLeft", "bottomRight"];
const cropEdges: CanvasCropEdge[] = ["cropTop", "cropRight", "cropBottom", "cropLeft"];

describe("preview position units", () => {
  it("scales position offsets from output pixels to the canvas with container width units", () => {
    // 192 of 1920 output pixels is 10% of the canvas width. 108 of 1080 is 10% of the canvas
    // height, which is 5.625% of the canvas width at 16:9.
    expect(previewMotionStyle({ ...baseLayer(), positionX: 192, positionY: -108 }, output).transform).toBe(
      "translate(10cqw, -5.625cqw) scale(1) rotate(0deg)",
    );
    expect(
      previewMotionStyle({ ...baseLayer(), centerX: 0.5, positionX: -64, positionY: 360 }, { width: 1280, height: 720 }).transform,
    ).toBe("translate(-50%, -50%) translate(-5cqw, 28.125cqw) scale(1) rotate(0deg)");
  });

  it("uses the render size as the output size, with a 16:9 fallback for invalid sizes", () => {
    expect([
      previewOutputSize({ width: 1080, height: 1920 }),
      previewOutputSize({ width: 0, height: 1080 }),
      previewOutputSize({ width: Number.NaN, height: 720 }),
    ]).toEqual([
      { width: 1080, height: 1920 },
      { width: 1920, height: 1080 },
      { width: 1920, height: 1080 },
    ]);
  });
});

describe("preview canvas geometry characterization", () => {
  it("finds timeline items by id across tracks", () => {
    const project = fixtureProject();
    expect(
      ["item-1", "caption-2", "sample-generated-clip", "missing"].map((itemId) =>
        findTimelineItem(project.timeline, itemId)?.id ?? null,
      ),
    ).toMatchInlineSnapshot(`
      [
        "item-1",
        "caption-2",
        "sample-generated-clip",
        null,
      ]
    `);
  });

  it("derives motion styles for fixture layers at 0, 1 and 3 seconds", () => {
    const project = fixtureProject();
    expect(
      [0, 1, 3].map((seconds) =>
        frameAt(project, seconds).layers.map((layer) => [layer.itemId, previewMotionStyle(layer, output)]),
      ),
    ).toMatchInlineSnapshot(`
      [
        [
          [
            "item-1",
            {
              "clipPath": undefined,
              "mixBlendMode": "normal",
              "opacity": 1,
              "transform": "translate(0cqw, 0cqw) scale(1) rotate(0deg)",
              "transformOrigin": "center center",
            },
          ],
        ],
        [
          [
            "item-1",
            {
              "clipPath": undefined,
              "mixBlendMode": "normal",
              "opacity": 1,
              "transform": "translate(0cqw, 0cqw) scale(1) rotate(0deg)",
              "transformOrigin": "center center",
            },
          ],
        ],
        [
          [
            "item-1",
            {
              "clipPath": undefined,
              "mixBlendMode": "normal",
              "opacity": 1,
              "transform": "translate(0cqw, 0cqw) scale(1) rotate(0deg)",
              "transformOrigin": "center center",
            },
          ],
        ],
      ]
    `);
  });

  it("derives motion styles for rotated, cropped, transparent, keyframed layers", () => {
    const project = transformedProject();
    const layers = [0, 1, 3].map((seconds) =>
      requiredValue(frameAt(project, seconds).layers[0], `transformed layer at ${seconds}s`),
    );
    expect(layers.map((layer) => previewMotionStyle(layer, output))).toMatchInlineSnapshot(`
      [
        {
          "clipPath": "inset(10% 0% 0% 20%)",
          "height": "40%",
          "left": "25%",
          "mixBlendMode": "plus-lighter",
          "opacity": 0,
          "top": "75%",
          "transform": "translate(-50%, -50%) translate(0cqw, 0cqw) scale(-1, 1) scale(1) rotate(-90deg)",
          "transformOrigin": "center center",
          "width": "50%",
        },
        {
          "clipPath": "inset(10% 0% 0% 20%)",
          "height": "40%",
          "left": "25%",
          "mixBlendMode": "plus-lighter",
          "opacity": 0,
          "top": "75%",
          "transform": "translate(-50%, -50%) translate(2.6042cqw, 0cqw) scale(-1, 1) scale(1.5) rotate(0deg)",
          "transformOrigin": "center center",
          "width": "50%",
        },
        {
          "clipPath": "inset(10% 0% 0% 20%)",
          "height": "40%",
          "left": "25%",
          "mixBlendMode": "plus-lighter",
          "opacity": 0,
          "top": "75%",
          "transform": "translate(-50%, -50%) translate(5.2083cqw, 0cqw) scale(-1, 1) scale(2) rotate(90deg)",
          "transformOrigin": "center center",
          "width": "50%",
        },
      ]
    `);
    expect(
      previewMotionStyle({
        opacity: 0.5,
        positionX: -4,
        positionY: 8,
        scale: 1,
        scaleX: 1.5,
        scaleY: 0.5,
        rotationDegrees: -45,
        flipVertical: true,
        cropBottom: 0.25,
      }, output),
    ).toMatchInlineSnapshot(`
      {
        "clipPath": "inset(0% 0% 25% 0%)",
        "mixBlendMode": "normal",
        "opacity": 0.5,
        "transform": "translate(-0.2083cqw, 0.4167cqw) scale(1, -1) scale(1.5, 0.5) rotate(-45deg)",
        "transformOrigin": "center center",
      }
    `);
  });

  it("clamps, rounds and canonicalizes canvas values", () => {
    expect({
      minimumCanvasItemSize,
      minimumVisibleCropFraction,
      clamp: [
        clampCanvasValue(-1, 0, 1),
        clampCanvasValue(0.5, 0, 1),
        clampCanvasValue(2, 0, 1),
        clampCanvasValue(Number.NaN, 0, 1),
      ],
      rounded: [roundedCanvasValue(0.123456), roundedCanvasValue(1 / 3), roundedCanvasValue(-0.00004)],
    }).toMatchInlineSnapshot(`
      {
        "clamp": [
          0,
          0.5,
          1,
          NaN,
        ],
        "minimumCanvasItemSize": 0.05,
        "minimumVisibleCropFraction": 0.05,
        "rounded": [
          0.1235,
          0.3333,
          -0,
        ],
      }
    `);
    const layer = baseLayer();
    const transformed = requiredValue(frameAt(transformedProject(), 1).layers[0], "transformed layer");
    expect([
      canonicalCanvasTransform(layer),
      canonicalCanvasCrop(layer),
      canonicalCanvasTransform(transformed),
      canonicalCanvasCrop(transformed),
    ]).toMatchInlineSnapshot(`
      [
        {
          "centerX": 0.5,
          "centerY": 0.5,
          "height": 1,
          "width": 1,
        },
        {
          "cropBottom": 0,
          "cropLeft": 0,
          "cropRight": 0,
          "cropTop": 0,
        },
        {
          "centerX": 0.25,
          "centerY": 0.75,
          "height": 0.4,
          "width": 0.5,
        },
        {
          "cropBottom": 0,
          "cropLeft": 0.2,
          "cropRight": 0,
          "cropTop": 0.1,
        },
      ]
    `);
  });

  it("normalizes rotation degrees", () => {
    expect(
      [-720, -181, -180, 0, 180, 181, 725].map((value) => [value, normalizeRotationDegrees(value)]),
    ).toMatchInlineSnapshot(`
      [
        [
          -720,
          0,
        ],
        [
          -181,
          179,
        ],
        [
          -180,
          -180,
        ],
        [
          0,
          0,
        ],
        [
          180,
          180,
        ],
        [
          181,
          -179,
        ],
        [
          725,
          5,
        ],
      ]
    `);
  });

  it("detects crop interactions", () => {
    const interactions: CanvasInteraction[] = [
      transformInteraction("move"),
      transformInteraction("topLeft"),
      {
        itemId: "item-1",
        pointerId: 1,
        mode: "rotate",
        pointerStartX: 0,
        pointerStartY: 0,
        centerClientX: 0,
        centerClientY: 0,
        initialPointerAngle: 0,
        initialRotationDegrees: 0,
        currentRotationDegrees: 0,
        moved: false,
      },
      ...cropEdges.map((edge) =>
        cropInteraction(edge, { cropTop: 0, cropRight: 0, cropBottom: 0, cropLeft: 0 }),
      ),
    ];
    expect(
      interactions.map((interaction) => [interaction.mode, isCanvasCropInteraction(interaction)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "move",
          false,
        ],
        [
          "topLeft",
          false,
        ],
        [
          "rotate",
          false,
        ],
        [
          "cropTop",
          true,
        ],
        [
          "cropRight",
          true,
        ],
        [
          "cropBottom",
          true,
        ],
        [
          "cropLeft",
          true,
        ],
      ]
    `);
  });

  it("resizes canvas transforms for every handle", () => {
    const layer = baseLayer();
    const rotated = { ...layer, rotationDegrees: 90, scaleX: -2, scaleY: 0.5 };
    const deltas: Array<[string, number, number]> = [
      ["positive", 100, 50],
      ["negative", -100, -50],
      ["below minimum", -1000, -1000],
      ["past maximum", 2000, 2000],
    ];
    expect(
      corners.map((corner) => [
        corner,
        deltas.map(([label, x, y]) => [
          label,
          resizedCanvasTransform(transformInteraction(corner), x, y, layer),
        ]),
        resizedCanvasTransform(transformInteraction(corner), 100, 50, rotated),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "topLeft",
          [
            [
              "positive",
              {
                "centerX": 0.55,
                "centerY": 0.55,
                "height": 0.4,
                "width": 0.4,
              },
            ],
            [
              "negative",
              {
                "centerX": 0.45,
                "centerY": 0.45,
                "height": 0.6,
                "width": 0.6,
              },
            ],
            [
              "below minimum",
              {
                "centerX": 0.25,
                "centerY": 0.25,
                "height": 1,
                "width": 1,
              },
            ],
            [
              "past maximum",
              {
                "centerX": 0.725,
                "centerY": 0.725,
                "height": 0.05,
                "width": 0.05,
              },
            ],
          ],
          {
            "centerX": 0.55,
            "centerY": 0.55,
            "height": 0.9,
            "width": 0.475,
          },
        ],
        [
          "topRight",
          [
            [
              "positive",
              {
                "centerX": 0.55,
                "centerY": 0.55,
                "height": 0.4,
                "width": 0.6,
              },
            ],
            [
              "negative",
              {
                "centerX": 0.45,
                "centerY": 0.45,
                "height": 0.6,
                "width": 0.4,
              },
            ],
            [
              "below minimum",
              {
                "centerX": 0.275,
                "centerY": 0.25,
                "height": 1,
                "width": 0.05,
              },
            ],
            [
              "past maximum",
              {
                "centerX": 0.75,
                "centerY": 0.725,
                "height": 0.05,
                "width": 1,
              },
            ],
          ],
          {
            "centerX": 0.55,
            "centerY": 0.55,
            "height": 0.9,
            "width": 0.525,
          },
        ],
        [
          "bottomLeft",
          [
            [
              "positive",
              {
                "centerX": 0.55,
                "centerY": 0.55,
                "height": 0.6,
                "width": 0.4,
              },
            ],
            [
              "negative",
              {
                "centerX": 0.45,
                "centerY": 0.45,
                "height": 0.4,
                "width": 0.6,
              },
            ],
            [
              "below minimum",
              {
                "centerX": 0.25,
                "centerY": 0.275,
                "height": 0.05,
                "width": 1,
              },
            ],
            [
              "past maximum",
              {
                "centerX": 0.725,
                "centerY": 0.75,
                "height": 1,
                "width": 0.05,
              },
            ],
          ],
          {
            "centerX": 0.55,
            "centerY": 0.55,
            "height": 0.1,
            "width": 0.475,
          },
        ],
        [
          "bottomRight",
          [
            [
              "positive",
              {
                "centerX": 0.55,
                "centerY": 0.55,
                "height": 0.6,
                "width": 0.6,
              },
            ],
            [
              "negative",
              {
                "centerX": 0.45,
                "centerY": 0.45,
                "height": 0.4,
                "width": 0.4,
              },
            ],
            [
              "below minimum",
              {
                "centerX": 0.275,
                "centerY": 0.275,
                "height": 0.05,
                "width": 0.05,
              },
            ],
            [
              "past maximum",
              {
                "centerX": 0.75,
                "centerY": 0.75,
                "height": 1,
                "width": 1,
              },
            ],
          ],
          {
            "centerX": 0.55,
            "centerY": 0.55,
            "height": 0.1,
            "width": 0.525,
          },
        ],
      ]
    `);
  });

  it("crops canvas edges at and past the minimum visible fraction", () => {
    const layer = baseLayer();
    const open = { cropTop: 0, cropRight: 0, cropBottom: 0, cropLeft: 0 };
    const nearlyClosed = { cropTop: 0.45, cropRight: 0.45, cropBottom: 0.45, cropLeft: 0.45 };
    const pointer = (edge: CanvasCropEdge, amount: number): [number, number] => {
      if (edge === "cropLeft") return [amount * 1000, 0];
      if (edge === "cropRight") return [-amount * 1000, 0];
      if (edge === "cropTop") return [0, amount * 500];
      return [0, -amount * 500];
    };
    expect(
      cropEdges.map((edge) => [
        edge,
        {
          small: croppedCanvasEdges(cropInteraction(edge, open), ...pointer(edge, 0.1), layer),
          atMinimum: croppedCanvasEdges(cropInteraction(edge, nearlyClosed), ...pointer(edge, 0.05), layer),
          pastMinimum: croppedCanvasEdges(cropInteraction(edge, nearlyClosed), ...pointer(edge, 0.5), layer),
          negative: croppedCanvasEdges(cropInteraction(edge, open), ...pointer(edge, -0.2), layer),
          rotated: croppedCanvasEdges(
            cropInteraction(edge, open),
            ...pointer(edge, 0.1),
            { ...layer, rotationDegrees: 90, width: 0.5, scaleY: 2 },
          ),
        },
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "cropTop",
          {
            "atMinimum": {
              "cropBottom": 0.45,
              "cropLeft": 0.45,
              "cropRight": 0.45,
              "cropTop": 0.5,
            },
            "negative": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "pastMinimum": {
              "cropBottom": 0.45,
              "cropLeft": 0.45,
              "cropRight": 0.45,
              "cropTop": 0.5,
            },
            "rotated": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "small": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0.1,
            },
          },
        ],
        [
          "cropRight",
          {
            "atMinimum": {
              "cropBottom": 0.45,
              "cropLeft": 0.45,
              "cropRight": 0.5,
              "cropTop": 0.45,
            },
            "negative": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "pastMinimum": {
              "cropBottom": 0.45,
              "cropLeft": 0.45,
              "cropRight": 0.5,
              "cropTop": 0.45,
            },
            "rotated": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "small": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0.1,
              "cropTop": 0,
            },
          },
        ],
        [
          "cropBottom",
          {
            "atMinimum": {
              "cropBottom": 0.5,
              "cropLeft": 0.45,
              "cropRight": 0.45,
              "cropTop": 0.45,
            },
            "negative": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "pastMinimum": {
              "cropBottom": 0.5,
              "cropLeft": 0.45,
              "cropRight": 0.45,
              "cropTop": 0.45,
            },
            "rotated": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "small": {
              "cropBottom": 0.1,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
          },
        ],
        [
          "cropLeft",
          {
            "atMinimum": {
              "cropBottom": 0.45,
              "cropLeft": 0.5,
              "cropRight": 0.45,
              "cropTop": 0.45,
            },
            "negative": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "pastMinimum": {
              "cropBottom": 0.45,
              "cropLeft": 0.5,
              "cropRight": 0.45,
              "cropTop": 0.45,
            },
            "rotated": {
              "cropBottom": 0,
              "cropLeft": 0,
              "cropRight": 0,
              "cropTop": 0,
            },
            "small": {
              "cropBottom": 0,
              "cropLeft": 0.1,
              "cropRight": 0,
              "cropTop": 0,
            },
          },
        ],
      ]
    `);
  });

  it.each([
    ["lower-third", false, true],
    ["punchy-caption", false, true],
    ["metric-callout", true, false],
    ["chapter-card", true, true],
    ["tracking-highlight", true, true],
    ["holographic-logo", true, true],
    ["gradient-background-loop", true, true],
  ] as const)(
    "maps %s template preview to exhaustive lane occupancy",
    (variant, topOccupied, bottomOccupied) => {
      expect(motionTemplatePreviewLaneOccupancy(variant)).toEqual({
        topOccupied,
        bottomOccupied,
      });
    },
  );
});
