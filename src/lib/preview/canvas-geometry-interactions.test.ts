import { describe, expect, it } from "vitest";
import {
  canvasInteractionMoved,
  canvasLayerCenterClient,
  keyboardCroppedEdges,
  keyboardMovedCanvasTransform,
  keyboardResizedCanvasTransform,
  keyboardRotatedDegrees,
  movedCanvasTransform,
  rotatedCanvasDegrees,
  type CanvasRotateInteraction,
  type CanvasTransformInteraction,
} from "@/lib/preview/canvas-geometry";
import { buildTimelinePreviewFrame, type TimelinePreviewLayer } from "@/lib/timeline-preview";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { requiredValue } from "@/test-utils/required";

function layer(overrides: Partial<TimelinePreviewLayer> = {}): TimelinePreviewLayer {
  const project = fixtureProject();
  const base = buildTimelinePreviewFrame({ timeline: project.timeline, media: project.media, playheadSeconds: 1 }).layers[0];
  return { ...requiredValue(base, "fixture layer"), ...overrides };
}

const box = { centerX: 0.5, centerY: 0.5, width: 0.5, height: 0.5 };

function moveInteraction(): CanvasTransformInteraction {
  return { itemId: "item-1", pointerId: 1, mode: "move", pointerStartX: 100, pointerStartY: 100, viewportWidth: 800, viewportHeight: 450, initial: box, current: box, moved: false };
}

describe("canvas pointer interactions", () => {
  it("counts a drag as moved from 2 px on either axis, and keeps it moved", () => {
    const interaction = moveInteraction();
    expect(canvasInteractionMoved(interaction, 101.9, 101.9)).toBe(false);
    expect(canvasInteractionMoved(interaction, 102, 100)).toBe(true);
    expect(canvasInteractionMoved(interaction, 100, 98)).toBe(true);
    expect(canvasInteractionMoved({ ...interaction, moved: true }, 100, 100)).toBe(true);
  });

  it("moves the center by the pointer delta as a canvas fraction, clamped to the canvas", () => {
    expect(movedCanvasTransform(moveInteraction(), 180, 145)).toEqual({ centerX: 0.6, centerY: 0.6, width: 0.5, height: 0.5 });
    expect(movedCanvasTransform(moveInteraction(), -900, 2000)).toEqual({ centerX: 0, centerY: 1, width: 0.5, height: 0.5 });
  });

  it("rotates by the pointer angle around the center and snaps to 15 degrees with Shift", () => {
    const interaction: CanvasRotateInteraction = {
      itemId: "item-1",
      pointerId: 1,
      mode: "rotate",
      pointerStartX: 200,
      pointerStartY: 100,
      centerClientX: 100,
      centerClientY: 100,
      initialPointerAngle: 0,
      initialRotationDegrees: 10,
      currentRotationDegrees: 10,
      moved: false,
    };
    const at = (degrees: number) => [100 + 100 * Math.cos((degrees * Math.PI) / 180), 100 + 100 * Math.sin((degrees * Math.PI) / 180)] as const;
    expect(rotatedCanvasDegrees(interaction, ...at(20), false)).toBeCloseTo(30, 3);
    expect(rotatedCanvasDegrees(interaction, ...at(26), true)).toBe(30);
    expect(rotatedCanvasDegrees(interaction, ...at(180), false)).toBeCloseTo(-170, 3);
  });

  it("finds the layer center in client pixels with position scaled from output pixels", () => {
    const bounds = { left: 10, top: 20, width: 960, height: 540 };
    expect(canvasLayerCenterClient(layer({ centerX: 0.25, centerY: 0.5, positionX: 192, positionY: -108 }), bounds, { width: 1920, height: 1080 })).toEqual({
      x: 10 + 240 + 96,
      y: 20 + 270 - 54,
    });
  });
});

describe("canvas keyboard interactions", () => {
  it("moves the center 0.01 per arrow key and ignores other keys", () => {
    const moved = layer({ centerX: 0.5, centerY: 0.995, width: 0.5, height: 0.5 });
    expect(keyboardMovedCanvasTransform(moved, "ArrowLeft")).toEqual({ centerX: 0.49, centerY: 0.995, width: 0.5, height: 0.5 });
    expect(keyboardMovedCanvasTransform(moved, "ArrowDown")).toEqual({ centerX: 0.5, centerY: 1, width: 0.5, height: 0.5 });
    expect(keyboardMovedCanvasTransform(moved, "Enter")).toBeNull();
  });

  it("resizes 1% of the canvas from a corner", () => {
    const sized = layer({ centerX: 0.5, centerY: 0.5, width: 0.5, height: 0.5 });
    expect(keyboardResizedCanvasTransform(sized, "bottomRight", "ArrowRight")).toEqual({ centerX: 0.505, centerY: 0.5, width: 0.51, height: 0.5 });
    expect(keyboardResizedCanvasTransform(sized, "topLeft", "ArrowUp")).toEqual({ centerX: 0.5, centerY: 0.495, width: 0.5, height: 0.51 });
    expect(keyboardResizedCanvasTransform(sized, "topLeft", "Tab")).toBeNull();
  });

  it("rotates 1 degree, or 15 with Shift, on ArrowLeft and ArrowRight only", () => {
    expect(keyboardRotatedDegrees(0, "ArrowLeft", false)).toBe(-1);
    expect(keyboardRotatedDegrees(175, "ArrowRight", true)).toBe(-170);
    expect(keyboardRotatedDegrees(0, "ArrowUp", false)).toBeNull();
  });

  it("moves a crop edge 0.01, or 0.05 with Shift, keeping the minimum visible fraction", () => {
    const crop = { cropTop: 0, cropRight: 0.1, cropBottom: 0, cropLeft: 0.8 };
    expect(keyboardCroppedEdges(crop, "cropTop", "ArrowDown", false)).toEqual({ ...crop, cropTop: 0.01 });
    expect(keyboardCroppedEdges(crop, "cropRight", "ArrowLeft", true)).toEqual({ ...crop, cropRight: 0.15 });
    expect(keyboardCroppedEdges(crop, "cropLeft", "ArrowRight", true)).toEqual({ ...crop, cropLeft: 0.85 });
    expect(keyboardCroppedEdges(crop, "cropBottom", "ArrowDown", false)).toEqual(crop);
    expect(keyboardCroppedEdges(crop, "cropTop", "ArrowLeft", false)).toBeNull();
  });
});
