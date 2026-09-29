import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import {
  canonicalCanvasCrop,
  canonicalCanvasTransform,
  canvasInteractionMoved,
  canvasLayerCenterClient,
  croppedCanvasEdges,
  findTimelineItem,
  isCanvasCropInteraction,
  keyboardCroppedEdges,
  keyboardMovedCanvasTransform,
  keyboardResizedCanvasTransform,
  keyboardRotatedDegrees,
  movedCanvasTransform,
  resizedCanvasTransform,
  rotatedCanvasDegrees,
  type CanvasCropEdge,
  type CanvasInteraction,
  type CanvasResizeCorner,
  type CanvasTransformInteraction,
  type PreviewOutputSize,
} from "@/lib/preview/canvas-geometry";
import type { ProjectAction } from "@/lib/project";
import { cropActions, motionActions, transformAction } from "@/lib/properties/visual-properties";
import { topmostTimelinePreviewLayerAtPoint, type TimelinePreviewLayer } from "@/lib/timeline-preview";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { usePropertyCommit } from "../properties/use-property-commit";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import type { CanvasLayerOverride } from "./canvas-editing";
import { CanvasCropControls, CanvasTransformControls } from "./canvas-selection-controls";

const cropOverrideKeys = ["cropTop", "cropRight", "cropBottom", "cropLeft"] as const;

interface CanvasSelectionProps {
  /** Hit-testable layers in draw order, with the current override applied. */
  readonly layers: readonly TimelinePreviewLayer[];
  readonly editableItemIds: ReadonlySet<string>;
  readonly outputSize: PreviewOutputSize;
  readonly override: CanvasLayerOverride | null;
  readonly onOverride: (override: CanvasLayerOverride | null) => void;
}

function surfaceBounds(element: HTMLElement | null) {
  const bounds = element?.getBoundingClientRect();
  return { left: bounds?.left ?? 0, top: bounds?.top ?? 0, width: Math.max(1, bounds?.width ?? 1), height: Math.max(1, bounds?.height ?? 1) };
}

function interactionPatch(interaction: CanvasInteraction): CanvasLayerOverride["patch"] {
  if (interaction.mode === "rotate") return { rotationDegrees: interaction.currentRotationDegrees };
  return interaction.current;
}

/**
 * Canvas selection over the composition: click to select the topmost visual layer (empty canvas
 * clears the selection), drag to move, scale or rotate, and crop mode. Drags show through the
 * override and commit once on release when the pointer moved; keyboard steps commit per key.
 * Crop mode keeps a draft override until Enter or an outside click applies it.
 */
export function CanvasSelection({ layers, editableItemIds, outputSize, override, onOverride }: CanvasSelectionProps) {
  const store = useEditorStoreApi();
  const { commit } = usePropertyCommit();
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const cropModeItemId = useEditorStore((state) => state.cropModeItemId);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const [interaction, setInteraction] = useState<CanvasInteraction | null>(null);
  // Bumped whenever the override changes, so a commit settling late does not clear a newer drag.
  const overrideVersion = useRef(0);
  // An outside click that ended crop mode must not also select on the canvas.
  const suppressSurfacePress = useRef(false);

  const selectedId = selectedItemIds.length === 1 ? selectedItemIds[0] : undefined;
  const selectedLayer = layers.find((layer) => layer.itemId === selectedId && editableItemIds.has(layer.itemId)) ?? null;
  const cropping = selectedLayer !== null && cropModeItemId === selectedLayer.itemId;

  // Crop mode ends when its layer is no longer the selected editable layer on the canvas.
  useEffect(() => {
    if (cropModeItemId === null || cropping) return;
    store.getState().setCropModeItemId(null);
    onOverride(null);
  }, [cropModeItemId, cropping, onOverride, store]);

  function setOverride(next: CanvasLayerOverride | null) {
    overrideVersion.current += 1;
    onOverride(next);
  }

  /** Commits one undo step; the override stays until the project shows the result. */
  function commitWithOverride(result: CommandResult | readonly ProjectAction[]) {
    const version = overrideVersion.current;
    void Promise.resolve(commit(result)).finally(() => {
      if (overrideVersion.current === version) setOverride(null);
    });
  }

  function projectItem(itemId: string) {
    const { project } = store.getState();
    return findTimelineItem(project.timeline, itemId);
  }

  function commitTransform(itemId: string, transform: CanvasTransformInteraction["current"]) {
    commitWithOverride(transformAction(itemId, transform));
  }

  function commitRotation(itemId: string, rotationDegrees: number) {
    const item = projectItem(itemId);
    if (item) commitWithOverride(motionActions(item, "rotationDegrees", rotationDegrees, store.getState().playheadSeconds));
  }

  function beginPointer(event: PointerEvent<HTMLElement>, layer: TimelinePreviewLayer) {
    if (event.button !== 0 || !editableItemIds.has(layer.itemId)) return false;
    event.preventDefault();
    event.stopPropagation();
    // preventDefault also stops the press from focusing: focus the pressed control, or the viewport
    // region for a canvas press, so keyboard steps and preview keys keep working.
    event.currentTarget.closest<HTMLElement>("button, [tabindex]")?.focus({ preventScroll: true });
    surfaceRef.current?.setPointerCapture?.(event.pointerId);
    return true;
  }

  function startTransform(event: PointerEvent<HTMLElement>, layer: TimelinePreviewLayer, mode: CanvasTransformInteraction["mode"]) {
    if (!beginPointer(event, layer)) return;
    const bounds = surfaceBounds(surfaceRef.current);
    const initial = canonicalCanvasTransform(layer);
    setInteraction({
      itemId: layer.itemId,
      pointerId: event.pointerId,
      mode,
      pointerStartX: event.clientX,
      pointerStartY: event.clientY,
      viewportWidth: bounds.width,
      viewportHeight: bounds.height,
      initial,
      current: initial,
      moved: false,
    });
  }

  function startRotate(event: PointerEvent<HTMLElement>, layer: TimelinePreviewLayer) {
    if (!beginPointer(event, layer)) return;
    const center = canvasLayerCenterClient(layer, surfaceBounds(surfaceRef.current), outputSize);
    setInteraction({
      itemId: layer.itemId,
      pointerId: event.pointerId,
      mode: "rotate",
      pointerStartX: event.clientX,
      pointerStartY: event.clientY,
      centerClientX: center.x,
      centerClientY: center.y,
      initialPointerAngle: Math.atan2(event.clientY - center.y, event.clientX - center.x),
      initialRotationDegrees: layer.rotationDegrees,
      currentRotationDegrees: layer.rotationDegrees,
      moved: false,
    });
  }

  function startCrop(event: PointerEvent<HTMLElement>, layer: TimelinePreviewLayer, edge: CanvasCropEdge) {
    if (!beginPointer(event, layer)) return;
    const bounds = surfaceBounds(surfaceRef.current);
    const initial = canonicalCanvasCrop(layer);
    setInteraction({
      itemId: layer.itemId,
      pointerId: event.pointerId,
      mode: edge,
      pointerStartX: event.clientX,
      pointerStartY: event.clientY,
      viewportWidth: bounds.width,
      viewportHeight: bounds.height,
      initial,
      current: initial,
      moved: false,
    });
  }

  function layerAt(event: MouseEvent<HTMLElement>) {
    const bounds = surfaceBounds(surfaceRef.current);
    const point = { x: (event.clientX - bounds.left) / bounds.width, y: (event.clientY - bounds.top) / bounds.height };
    // Position offsets are output pixels, so hit-testing measures in the render size.
    return topmostTimelinePreviewLayerAtPoint(layers, point, outputSize);
  }

  function pressSurface(event: PointerEvent<HTMLDivElement>) {
    if (suppressSurfacePress.current) {
      suppressSurfacePress.current = false;
      return;
    }
    if (event.button !== 0 || interaction) return;
    const layer = layerAt(event);
    const state = store.getState();
    if (!layer) {
      state.clearSelection();
      return;
    }
    state.selectItems([layer.itemId]);
    startTransform(event, layer, "move");
  }

  function moveOnSurface(event: PointerEvent<HTMLDivElement>) {
    if (!interaction || event.pointerId !== interaction.pointerId) return;
    const layer = layers.find((candidate) => candidate.itemId === interaction.itemId);
    if (!layer) return;
    event.preventDefault();
    const moved = canvasInteractionMoved(interaction, event.clientX, event.clientY);
    let next: CanvasInteraction;
    if (interaction.mode === "rotate") {
      next = { ...interaction, moved, currentRotationDegrees: rotatedCanvasDegrees(interaction, event.clientX, event.clientY, event.shiftKey) };
    } else if (isCanvasCropInteraction(interaction)) {
      next = { ...interaction, moved, current: croppedCanvasEdges(interaction, event.clientX, event.clientY, layer) };
    } else {
      const current =
        interaction.mode === "move"
          ? movedCanvasTransform(interaction, event.clientX, event.clientY)
          : resizedCanvasTransform(interaction, event.clientX, event.clientY, layer);
      next = { ...interaction, moved, current };
    }
    setInteraction(next);
    setOverride({ itemId: next.itemId, patch: interactionPatch(next) });
  }

  function releaseSurface(event: PointerEvent<HTMLDivElement>) {
    if (!interaction || event.pointerId !== interaction.pointerId) return;
    event.preventDefault();
    surfaceRef.current?.releasePointerCapture?.(event.pointerId);
    setInteraction(null);
    if (isCanvasCropInteraction(interaction)) {
      // Crop drags only change the draft; crop mode applies it.
      if (!interaction.moved) setOverride({ itemId: interaction.itemId, patch: interaction.initial });
      return;
    }
    if (!interaction.moved) {
      if (override?.itemId === interaction.itemId) setOverride(null);
      return;
    }
    if (interaction.mode === "rotate") commitRotation(interaction.itemId, interaction.currentRotationDegrees);
    else commitTransform(interaction.itemId, interaction.current);
  }

  function cancelPointer() {
    if (!interaction) return;
    setInteraction(null);
    if (isCanvasCropInteraction(interaction)) setOverride({ itemId: interaction.itemId, patch: interaction.initial });
    else setOverride(null);
  }

  function enterCropMode(itemId: string) {
    const state = store.getState();
    state.selectItems([itemId]);
    state.setCropModeItemId(itemId);
  }

  function doubleClickSurface(event: MouseEvent<HTMLDivElement>) {
    const layer = layerAt(event);
    if (layer && editableItemIds.has(layer.itemId)) enterCropMode(layer.itemId);
  }

  function commitCropMode() {
    if (!selectedLayer) return;
    const hasDraft = override?.itemId === selectedLayer.itemId && cropOverrideKeys.some((key) => key in override.patch);
    const crop = canonicalCanvasCrop(selectedLayer);
    store.getState().setCropModeItemId(null);
    const item = projectItem(selectedLayer.itemId);
    if (hasDraft && item) commitWithOverride(cropActions(item, crop, store.getState().playheadSeconds));
    else setOverride(null);
  }

  function commitCropModeFromOutside(event?: Event) {
    if (event?.target instanceof Node && surfaceRef.current?.contains(event.target)) suppressSurfacePress.current = true;
    commitCropMode();
  }

  function cancelCropMode() {
    store.getState().setCropModeItemId(null);
    setOverride(null);
  }

  function keyMove(event: KeyboardEvent<HTMLButtonElement>, layer: TimelinePreviewLayer) {
    const next = keyboardMovedCanvasTransform(layer, event.key);
    if (!next) return;
    event.preventDefault();
    const initial = canonicalCanvasTransform(layer);
    if (next.centerX !== initial.centerX || next.centerY !== initial.centerY) commitTransform(layer.itemId, next);
  }

  function keyResize(event: KeyboardEvent<HTMLButtonElement>, layer: TimelinePreviewLayer, corner: CanvasResizeCorner) {
    const next = keyboardResizedCanvasTransform(layer, corner, event.key);
    if (!next) return;
    event.preventDefault();
    const initial = canonicalCanvasTransform(layer);
    if (next.width !== initial.width || next.height !== initial.height) commitTransform(layer.itemId, next);
  }

  function keyRotate(event: KeyboardEvent<HTMLButtonElement>, layer: TimelinePreviewLayer) {
    const next = keyboardRotatedDegrees(layer.rotationDegrees, event.key, event.shiftKey);
    if (next === null) return;
    event.preventDefault();
    commitRotation(layer.itemId, next);
  }

  function keyCrop(event: KeyboardEvent<HTMLButtonElement>, layer: TimelinePreviewLayer, edge: CanvasCropEdge) {
    const next = keyboardCroppedEdges(canonicalCanvasCrop(layer), edge, event.key, event.shiftKey);
    if (!next) return;
    event.preventDefault();
    setOverride({ itemId: layer.itemId, patch: next });
  }

  return (
    <>
      <div
        ref={surfaceRef}
        data-testid="preview-canvas-surface"
        className="absolute inset-0 z-[15] touch-none"
        onPointerDown={pressSurface}
        onPointerMove={moveOnSurface}
        onPointerUp={releaseSurface}
        onPointerCancel={cancelPointer}
        onDoubleClick={doubleClickSurface}
      />
      {selectedLayer && !cropping && (
        <CanvasTransformControls
          layer={selectedLayer}
          outputSize={outputSize}
          onMovePointerDown={(event) => startTransform(event, selectedLayer, "move")}
          onMoveKeyDown={(event) => keyMove(event, selectedLayer)}
          onMoveDoubleClick={() => enterCropMode(selectedLayer.itemId)}
          onRotatePointerDown={(event) => startRotate(event, selectedLayer)}
          onRotateKeyDown={(event) => keyRotate(event, selectedLayer)}
          onResizePointerDown={(event, corner) => startTransform(event, selectedLayer, corner)}
          onResizeKeyDown={(event, corner) => keyResize(event, selectedLayer, corner)}
        />
      )}
      {selectedLayer && cropping && (
        <CanvasCropControls
          layer={selectedLayer}
          outputSize={outputSize}
          onCropPointerDown={(event, edge) => startCrop(event, selectedLayer, edge)}
          onCropKeyDown={(event, edge) => keyCrop(event, selectedLayer, edge)}
          onCommit={commitCropModeFromOutside}
          onCancel={cancelCropMode}
        />
      )}
    </>
  );
}
