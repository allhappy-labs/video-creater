import type { CSSProperties } from "react";
import type { ProjectActionVisualCrop, ProjectActionVisualTransform } from "@/lib/project";
import type { Timeline, TimelineItem } from "@/lib/timeline";
import type { TimelinePreviewLayer } from "@/lib/timeline-preview";
import type { MotionTemplatePreviewVariant } from "@/lib/motion-templates";

export type TimelinePreviewIssueState = "clear" | "notice" | "retry";

export interface TimelinePreviewCanvasState {
  issueState: TimelinePreviewIssueState;
  topOccupied: boolean;
  bottomOccupied: boolean;
}

export function findTimelineItem(timeline: Timeline, itemId: string): TimelineItem | null {
  for (const track of timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) {
      return item;
    }
  }

  return null;
}

/** The render resolution; `positionX`/`positionY` are pixels at this size, as in the renderer. */
export interface PreviewOutputSize {
  readonly width: number;
  readonly height: number;
}

const fallbackOutputSize: PreviewOutputSize = { width: 1920, height: 1080 };

/**
 * The output size for position units and canvas hit-testing. Invalid render sizes fall back to
 * 16:9, the aspect ratio the preview canvas uses for them.
 */
export function previewOutputSize(renderSettings: { readonly width: number; readonly height: number }): PreviewOutputSize {
  const { width, height } = renderSettings;
  return Number.isFinite(width) && Number.isFinite(height) && width > 0 && height > 0 ? { width, height } : fallbackOutputSize;
}

/**
 * A position offset as a CSS length on the preview canvas. The renderer adds positionX/positionY as
 * output pixels, so the offset is a fraction of the render width expressed in `cqw` (the canvas is
 * an inline-size container). The canvas keeps the output aspect ratio, so the vertical offset is
 * also a fraction of the render width.
 */
export function canvasPositionOffset(outputPixels: number, output: PreviewOutputSize) {
  return `${roundedCanvasValue((outputPixels / output.width) * 100)}cqw`;
}

/** A canvas fraction as a CSS percentage, rounded so float noise does not reach the style. */
export function canvasPercent(fraction: number) {
  return `${roundedCanvasValue(fraction * 100)}%`;
}

/** CSS for a preview layer on the canvas; `output` is the render size that position offsets use. */
export function previewMotionStyle(layer: {
  opacity: number;
  blendMode?: "over" | "add";
  positionX: number;
  positionY: number;
  scale: number;
  scaleX: number;
  scaleY: number;
  rotationDegrees: number;
  centerX?: number;
  centerY?: number;
  width?: number;
  height?: number;
  flipHorizontal?: boolean;
  flipVertical?: boolean;
  cropTop?: number;
  cropRight?: number;
  cropBottom?: number;
  cropLeft?: number;
  colorGrade?: {
    exposure: number;
    contrast: number;
    saturation: number;
  };
}, output: PreviewOutputSize): CSSProperties {
  const scaleTransform =
    layer.scaleX === layer.scaleY ? `scale(${layer.scale})` : `scale(${layer.scaleX}, ${layer.scaleY})`;
  const hasCanvasTransform =
    layer.centerX !== undefined ||
    layer.centerY !== undefined ||
    layer.width !== undefined ||
    layer.height !== undefined;
  const flipTransform =
    layer.flipHorizontal || layer.flipVertical
      ? `scale(${layer.flipHorizontal ? -1 : 1}, ${layer.flipVertical ? -1 : 1}) `
      : "";
  return {
    opacity: layer.opacity,
    mixBlendMode: layer.blendMode === "add" ? "plus-lighter" : "normal",
    clipPath:
      layer.cropTop || layer.cropRight || layer.cropBottom || layer.cropLeft
        ? `inset(${canvasPercent(layer.cropTop ?? 0)} ${canvasPercent(layer.cropRight ?? 0)} ${canvasPercent(layer.cropBottom ?? 0)} ${canvasPercent(layer.cropLeft ?? 0)})`
        : undefined,
    ...(hasCanvasTransform
      ? {
          left: canvasPercent(layer.centerX ?? 0.5),
          top: canvasPercent(layer.centerY ?? 0.5),
          width: canvasPercent(layer.width ?? 1),
          height: canvasPercent(layer.height ?? 1),
        }
      : {}),
    transform: `${hasCanvasTransform ? "translate(-50%, -50%) " : ""}translate(${canvasPositionOffset(layer.positionX, output)}, ${canvasPositionOffset(layer.positionY, output)}) ${flipTransform}${scaleTransform} rotate(${layer.rotationDegrees}deg)`,
    transformOrigin: "center center",
  };
}

export type CanvasResizeCorner = "topLeft" | "topRight" | "bottomLeft" | "bottomRight";
export type CanvasCropEdge = "cropTop" | "cropRight" | "cropBottom" | "cropLeft";

export interface CanvasTransformInteraction {
  itemId: string;
  pointerId: number;
  mode: "move" | CanvasResizeCorner;
  pointerStartX: number;
  pointerStartY: number;
  viewportWidth: number;
  viewportHeight: number;
  initial: Required<Pick<ProjectActionVisualTransform, "centerX" | "centerY" | "width" | "height">>;
  current: Required<Pick<ProjectActionVisualTransform, "centerX" | "centerY" | "width" | "height">>;
  moved: boolean;
}

export interface CanvasRotateInteraction {
  itemId: string;
  pointerId: number;
  mode: "rotate";
  pointerStartX: number;
  pointerStartY: number;
  centerClientX: number;
  centerClientY: number;
  initialPointerAngle: number;
  initialRotationDegrees: number;
  currentRotationDegrees: number;
  moved: boolean;
}

export interface CanvasCropInteraction {
  itemId: string;
  pointerId: number;
  mode: CanvasCropEdge;
  pointerStartX: number;
  pointerStartY: number;
  viewportWidth: number;
  viewportHeight: number;
  initial: Required<ProjectActionVisualCrop>;
  current: Required<ProjectActionVisualCrop>;
  moved: boolean;
}

export type CanvasInteraction =
  | CanvasTransformInteraction
  | CanvasRotateInteraction
  | CanvasCropInteraction;

export function isCanvasCropInteraction(
  interaction: CanvasInteraction,
): interaction is CanvasCropInteraction {
  return (
    interaction.mode === "cropTop" ||
    interaction.mode === "cropRight" ||
    interaction.mode === "cropBottom" ||
    interaction.mode === "cropLeft"
  );
}

export const minimumCanvasItemSize = 0.05;
export const minimumVisibleCropFraction = 0.05;

export function clampCanvasValue(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}

export function roundedCanvasValue(value: number) {
  return Number(value.toFixed(4));
}

export function canonicalCanvasTransform(layer: TimelinePreviewLayer) {
  return {
    centerX: layer.centerX ?? 0.5,
    centerY: layer.centerY ?? 0.5,
    width: layer.width ?? 1,
    height: layer.height ?? 1,
  };
}

export function canonicalCanvasCrop(layer: TimelinePreviewLayer): Required<ProjectActionVisualCrop> {
  return {
    cropTop: layer.cropTop,
    cropRight: layer.cropRight,
    cropBottom: layer.cropBottom,
    cropLeft: layer.cropLeft,
  };
}

export function normalizeRotationDegrees(value: number) {
  let normalized = value % 360;
  if (normalized > 180) normalized -= 360;
  if (normalized < -180) normalized += 360;
  return roundedCanvasValue(normalized);
}

export function croppedCanvasEdges(
  interaction: CanvasCropInteraction,
  clientX: number,
  clientY: number,
  layer: TimelinePreviewLayer,
) {
  const deltaXPixels = clientX - interaction.pointerStartX;
  const deltaYPixels = clientY - interaction.pointerStartY;
  const radians = (-layer.rotationDegrees * Math.PI) / 180;
  const localDeltaXPixels = deltaXPixels * Math.cos(radians) - deltaYPixels * Math.sin(radians);
  const localDeltaYPixels = deltaXPixels * Math.sin(radians) + deltaYPixels * Math.cos(radians);
  const widthPixels = Math.max(
    1,
    (layer.width ?? 1) * Math.abs(layer.scaleX) * interaction.viewportWidth,
  );
  const heightPixels = Math.max(
    1,
    (layer.height ?? 1) * Math.abs(layer.scaleY) * interaction.viewportHeight,
  );
  const next = { ...interaction.initial };
  if (interaction.mode === "cropLeft") {
    next.cropLeft = clampCanvasValue(
      interaction.initial.cropLeft + localDeltaXPixels / widthPixels,
      0,
      1 - interaction.initial.cropRight - minimumVisibleCropFraction,
    );
  } else if (interaction.mode === "cropRight") {
    next.cropRight = clampCanvasValue(
      interaction.initial.cropRight - localDeltaXPixels / widthPixels,
      0,
      1 - interaction.initial.cropLeft - minimumVisibleCropFraction,
    );
  } else if (interaction.mode === "cropTop") {
    next.cropTop = clampCanvasValue(
      interaction.initial.cropTop + localDeltaYPixels / heightPixels,
      0,
      1 - interaction.initial.cropBottom - minimumVisibleCropFraction,
    );
  } else {
    next.cropBottom = clampCanvasValue(
      interaction.initial.cropBottom - localDeltaYPixels / heightPixels,
      0,
      1 - interaction.initial.cropTop - minimumVisibleCropFraction,
    );
  }
  return Object.fromEntries(
    Object.entries(next).map(([key, value]) => [key, roundedCanvasValue(value)]),
  ) as Required<ProjectActionVisualCrop>;
}

export function resizedCanvasTransform(
  interaction: CanvasTransformInteraction,
  clientX: number,
  clientY: number,
  layer: TimelinePreviewLayer,
) {
  const deltaXPixels = clientX - interaction.pointerStartX;
  const deltaYPixels = clientY - interaction.pointerStartY;
  const radians = (-layer.rotationDegrees * Math.PI) / 180;
  const localDeltaXPixels = deltaXPixels * Math.cos(radians) - deltaYPixels * Math.sin(radians);
  const localDeltaYPixels = deltaXPixels * Math.sin(radians) + deltaYPixels * Math.cos(radians);
  const deltaX = localDeltaXPixels / interaction.viewportWidth;
  const deltaY = localDeltaYPixels / interaction.viewportHeight;
  const horizontalSign = interaction.mode === "topLeft" || interaction.mode === "bottomLeft" ? -1 : 1;
  const verticalSign = interaction.mode === "topLeft" || interaction.mode === "topRight" ? -1 : 1;
  const scaleX = Math.max(0.01, Math.abs(layer.scaleX));
  const scaleY = Math.max(0.01, Math.abs(layer.scaleY));
  const width = clampCanvasValue(
    interaction.initial.width + (horizontalSign * deltaX) / scaleX,
    minimumCanvasItemSize,
    1,
  );
  const height = clampCanvasValue(
    interaction.initial.height + (verticalSign * deltaY) / scaleY,
    minimumCanvasItemSize,
    1,
  );
  const appliedWidthDelta = (width - interaction.initial.width) * scaleX;
  const appliedHeightDelta = (height - interaction.initial.height) * scaleY;
  const rotation = (layer.rotationDegrees * Math.PI) / 180;
  const localCenterShiftX = (horizontalSign * appliedWidthDelta * interaction.viewportWidth) / 2;
  const localCenterShiftY = (verticalSign * appliedHeightDelta * interaction.viewportHeight) / 2;
  const centerShiftX =
    localCenterShiftX * Math.cos(rotation) - localCenterShiftY * Math.sin(rotation);
  const centerShiftY =
    localCenterShiftX * Math.sin(rotation) + localCenterShiftY * Math.cos(rotation);

  return {
    centerX: roundedCanvasValue(
      clampCanvasValue(interaction.initial.centerX + centerShiftX / interaction.viewportWidth, 0, 1),
    ),
    centerY: roundedCanvasValue(
      clampCanvasValue(interaction.initial.centerY + centerShiftY / interaction.viewportHeight, 0, 1),
    ),
    width: roundedCanvasValue(width),
    height: roundedCanvasValue(height),
  };
}

/** Pointer travel on either axis before a press becomes a drag that commits. */
const canvasMoveThresholdPixels = 2;
const canvasRotationSnapDegrees = 15;
const canvasKeyboardStep = 0.01;
const canvasKeyboardCropLargeStep = 0.05;

type CanvasTransformValues = CanvasTransformInteraction["current"];
type CanvasCropValues = CanvasCropInteraction["current"];

/** `moved` becomes true once the pointer travels the threshold, and stays true. */
export function canvasInteractionMoved(
  interaction: Pick<CanvasInteraction, "pointerStartX" | "pointerStartY" | "moved">,
  clientX: number,
  clientY: number,
) {
  return (
    interaction.moved ||
    Math.abs(clientX - interaction.pointerStartX) >= canvasMoveThresholdPixels ||
    Math.abs(clientY - interaction.pointerStartY) >= canvasMoveThresholdPixels
  );
}

/** Move drag: the center follows the pointer as a fraction of the canvas, clamped to it. */
export function movedCanvasTransform(
  interaction: CanvasTransformInteraction,
  clientX: number,
  clientY: number,
): CanvasTransformValues {
  const deltaX = (clientX - interaction.pointerStartX) / interaction.viewportWidth;
  const deltaY = (clientY - interaction.pointerStartY) / interaction.viewportHeight;
  return {
    ...interaction.initial,
    centerX: roundedCanvasValue(clampCanvasValue(interaction.initial.centerX + deltaX, 0, 1)),
    centerY: roundedCanvasValue(clampCanvasValue(interaction.initial.centerY + deltaY, 0, 1)),
  };
}

/** Rotate drag: the pointer angle change around the layer center; `snap` (Shift) rounds to 15°. */
export function rotatedCanvasDegrees(
  interaction: CanvasRotateInteraction,
  clientX: number,
  clientY: number,
  snap: boolean,
) {
  const pointerAngle = Math.atan2(clientY - interaction.centerClientY, clientX - interaction.centerClientX);
  const deltaDegrees = ((pointerAngle - interaction.initialPointerAngle) * 180) / Math.PI;
  const unsnapped = normalizeRotationDegrees(interaction.initialRotationDegrees + deltaDegrees);
  return snap ? normalizeRotationDegrees(Math.round(unsnapped / canvasRotationSnapDegrees) * canvasRotationSnapDegrees) : unsnapped;
}

/** The layer center in client pixels; position offsets are output pixels scaled to the canvas. */
export function canvasLayerCenterClient(
  layer: TimelinePreviewLayer,
  bounds: { readonly left: number; readonly top: number; readonly width: number; readonly height: number },
  output: PreviewOutputSize,
) {
  return {
    x: bounds.left + (layer.centerX ?? 0.5) * bounds.width + (layer.positionX / output.width) * bounds.width,
    y: bounds.top + (layer.centerY ?? 0.5) * bounds.height + (layer.positionY / output.height) * bounds.height,
  };
}

function arrowDirection(key: string) {
  if (key === "ArrowLeft") return { horizontal: -1, vertical: 0 };
  if (key === "ArrowRight") return { horizontal: 1, vertical: 0 };
  if (key === "ArrowUp") return { horizontal: 0, vertical: -1 };
  if (key === "ArrowDown") return { horizontal: 0, vertical: 1 };
  return null;
}

/** Arrow keys on the move control: the center moves 0.01 of the canvas. Null for other keys. */
export function keyboardMovedCanvasTransform(layer: TimelinePreviewLayer, key: string): CanvasTransformValues | null {
  const direction = arrowDirection(key);
  if (!direction) return null;
  const initial = canonicalCanvasTransform(layer);
  return {
    ...initial,
    centerX: roundedCanvasValue(clampCanvasValue(initial.centerX + direction.horizontal * canvasKeyboardStep, 0, 1)),
    centerY: roundedCanvasValue(clampCanvasValue(initial.centerY + direction.vertical * canvasKeyboardStep, 0, 1)),
  };
}

/** Arrow keys on a corner handle: a 1% resize drag from that corner. Null for other keys. */
export function keyboardResizedCanvasTransform(
  layer: TimelinePreviewLayer,
  corner: CanvasResizeCorner,
  key: string,
): CanvasTransformValues | null {
  const direction = arrowDirection(key);
  if (!direction) return null;
  const initial = canonicalCanvasTransform(layer);
  // One pixel of a synthetic 100-pixel canvas.
  const interaction: CanvasTransformInteraction = {
    itemId: layer.itemId,
    pointerId: -1,
    mode: corner,
    pointerStartX: 0,
    pointerStartY: 0,
    viewportWidth: 100,
    viewportHeight: 100,
    initial,
    current: initial,
    moved: true,
  };
  return resizedCanvasTransform(interaction, direction.horizontal, direction.vertical, layer);
}

/** ArrowLeft/ArrowRight on the rotate control: 1°, or 15° with Shift. Null for other keys. */
export function keyboardRotatedDegrees(rotationDegrees: number, key: string, large: boolean) {
  if (key !== "ArrowLeft" && key !== "ArrowRight") return null;
  const step = large ? canvasRotationSnapDegrees : 1;
  return normalizeRotationDegrees(rotationDegrees + (key === "ArrowLeft" ? -step : step));
}

const oppositeCropEdge = {
  cropTop: "cropBottom",
  cropRight: "cropLeft",
  cropBottom: "cropTop",
  cropLeft: "cropRight",
} as const satisfies Record<CanvasCropEdge, CanvasCropEdge>;

/**
 * Arrow keys on a crop handle move that edge inward or outward by 0.01, or 0.05 with Shift, keeping
 * the minimum visible fraction. Null for keys that do not move the edge.
 */
export function keyboardCroppedEdges(
  crop: CanvasCropValues,
  edge: CanvasCropEdge,
  key: string,
  large: boolean,
): CanvasCropValues | null {
  const arrow = arrowDirection(key);
  if (!arrow) return null;
  const direction =
    edge === "cropLeft" ? arrow.horizontal : edge === "cropRight" ? -arrow.horizontal : edge === "cropTop" ? arrow.vertical : -arrow.vertical;
  if (direction === 0) return null;
  const step = large ? canvasKeyboardCropLargeStep : canvasKeyboardStep;
  const maximum = 1 - crop[oppositeCropEdge[edge]] - minimumVisibleCropFraction;
  return { ...crop, [edge]: roundedCanvasValue(clampCanvasValue(crop[edge] + direction * step, 0, maximum)) };
}

export interface MotionTemplatePreviewLaneOccupancy {
  readonly topOccupied: boolean;
  readonly bottomOccupied: boolean;
}

const motionTemplatePreviewLaneOccupancyByVariant = {
  "lower-third": { topOccupied: false, bottomOccupied: true },
  "punchy-caption": { topOccupied: false, bottomOccupied: true },
  "metric-callout": { topOccupied: true, bottomOccupied: false },
  "chapter-card": { topOccupied: true, bottomOccupied: true },
  "tracking-highlight": { topOccupied: true, bottomOccupied: true },
  "holographic-logo": { topOccupied: true, bottomOccupied: true },
  "gradient-background-loop": { topOccupied: true, bottomOccupied: true },
} as const satisfies Record<MotionTemplatePreviewVariant, MotionTemplatePreviewLaneOccupancy>;

export function motionTemplatePreviewLaneOccupancy(
  variant: MotionTemplatePreviewVariant,
): MotionTemplatePreviewLaneOccupancy {
  return motionTemplatePreviewLaneOccupancyByVariant[variant];
}
