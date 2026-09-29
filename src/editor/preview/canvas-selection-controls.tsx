import { Crop, RotateCw } from "lucide-react";
import { useEffect, useId, useRef, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { canvasPercent, canvasPositionOffset, type CanvasCropEdge, type CanvasResizeCorner, type PreviewOutputSize } from "@/lib/preview/canvas-geometry";
import type { TimelinePreviewLayer } from "@/lib/timeline-preview";
import { cn } from "@/lib/utils";

type ButtonPointerHandler = (event: PointerEvent<HTMLButtonElement>) => void;
type ButtonKeyHandler = (event: KeyboardEvent<HTMLButtonElement>) => void;

const handleFocusClassName = "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

const corners: readonly { readonly corner: CanvasResizeCorner; readonly label: string; readonly className: string }[] = [
  { corner: "topLeft", label: "top left", className: "left-0 top-0 cursor-nwse-resize" },
  { corner: "topRight", label: "top right", className: "right-0 top-0 cursor-nesw-resize" },
  { corner: "bottomLeft", label: "bottom left", className: "bottom-0 left-0 cursor-nesw-resize" },
  { corner: "bottomRight", label: "bottom right", className: "bottom-0 right-0 cursor-nwse-resize" },
];

const cropEdges: readonly { readonly edge: CanvasCropEdge; readonly label: string; readonly className: string; readonly style: (layer: TimelinePreviewLayer) => CSSProperties }[] = [
  { edge: "cropTop", label: "top", className: "left-1/2 -translate-x-1/2 -translate-y-1/2 cursor-row-resize", style: (layer) => ({ top: canvasPercent(layer.cropTop) }) },
  { edge: "cropRight", label: "right", className: "top-1/2 translate-x-1/2 -translate-y-1/2 cursor-col-resize", style: (layer) => ({ right: canvasPercent(layer.cropRight) }) },
  { edge: "cropBottom", label: "bottom", className: "left-1/2 -translate-x-1/2 translate-y-1/2 cursor-row-resize", style: (layer) => ({ bottom: canvasPercent(layer.cropBottom) }) },
  { edge: "cropLeft", label: "left", className: "top-1/2 -translate-x-1/2 -translate-y-1/2 cursor-col-resize", style: (layer) => ({ left: canvasPercent(layer.cropLeft) }) },
];

/**
 * The selection box over a layer: its canvas box at its scaled size, offset and rotated like the layer
 * (position offsets are output pixels in `cqw`, as in `previewMotionStyle`). Sizing by scale instead of
 * a CSS scale keeps the handles undistorted and matches canvas hit-testing.
 */
function selectionBoxStyle(layer: TimelinePreviewLayer, output: PreviewOutputSize): CSSProperties {
  return {
    left: canvasPercent(layer.centerX ?? 0.5),
    top: canvasPercent(layer.centerY ?? 0.5),
    width: canvasPercent((layer.width ?? 1) * Math.abs(layer.scaleX)),
    height: canvasPercent((layer.height ?? 1) * Math.abs(layer.scaleY)),
    transform: `translate(-50%, -50%) translate(${canvasPositionOffset(layer.positionX, output)}, ${canvasPositionOffset(layer.positionY, output)}) rotate(${layer.rotationDegrees}deg)`,
    transformOrigin: "center center",
  };
}

interface TransformControlsProps {
  readonly layer: TimelinePreviewLayer;
  readonly outputSize: PreviewOutputSize;
  readonly onMovePointerDown: ButtonPointerHandler;
  readonly onMoveKeyDown: ButtonKeyHandler;
  readonly onMoveDoubleClick: () => void;
  readonly onRotatePointerDown: ButtonPointerHandler;
  readonly onRotateKeyDown: ButtonKeyHandler;
  readonly onResizePointerDown: (event: PointerEvent<HTMLButtonElement>, corner: CanvasResizeCorner) => void;
  readonly onResizeKeyDown: (event: KeyboardEvent<HTMLButtonElement>, corner: CanvasResizeCorner) => void;
}

/**
 * Transform box for the selected visual layer. The move area sits below text and caption overlays so
 * they stay clickable; the outline, rotate and corner handles sit above them.
 */
export function CanvasTransformControls(props: TransformControlsProps) {
  const { layer, outputSize } = props;
  const style = selectionBoxStyle(layer, outputSize);
  return (
    <>
      <div className="pointer-events-none absolute z-[16]" style={style}>
        <button
          type="button"
          aria-label={`Move ${layer.label} in preview canvas`}
          title="Drag to move. Double-click to crop."
          className={cn("pointer-events-auto absolute inset-0 cursor-move touch-none bg-transparent", handleFocusClassName)}
          onPointerDown={props.onMovePointerDown}
          onKeyDown={props.onMoveKeyDown}
          onDoubleClick={props.onMoveDoubleClick}
        />
      </div>
      <div
        role="group"
        aria-label={`Canvas transform controls for ${layer.label}`}
        className="pointer-events-none absolute z-[25] border border-accent shadow-[0_0_0_1px_hsl(var(--background)/0.7)]"
        style={style}
      >
        <button
          type="button"
          aria-label={`Rotate ${layer.label} in preview canvas`}
          title="Drag to rotate. Hold Shift to snap to 15 degrees."
          className={cn(
            "pointer-events-auto absolute left-1/2 top-2 flex h-7 w-7 -translate-x-1/2 touch-none items-center justify-center rounded-full border border-accent bg-popover/90 text-foreground shadow-md",
            handleFocusClassName,
          )}
          onPointerDown={props.onRotatePointerDown}
          onKeyDown={props.onRotateKeyDown}
        >
          <RotateCw className="h-3.5 w-3.5" aria-hidden />
        </button>
        {corners.map(({ corner, label, className }) => (
          <button
            key={corner}
            type="button"
            aria-label={`Resize ${layer.label} from ${label}`}
            className={cn("pointer-events-auto absolute flex h-7 w-7 touch-none items-center justify-center rounded-full", handleFocusClassName, className)}
            onPointerDown={(event) => props.onResizePointerDown(event, corner)}
            onKeyDown={(event) => props.onResizeKeyDown(event, corner)}
          >
            <span className="h-2.5 w-2.5 rounded-sm border border-background bg-accent shadow-sm" />
          </button>
        ))}
      </div>
    </>
  );
}

interface CropControlsProps {
  readonly layer: TimelinePreviewLayer;
  readonly outputSize: PreviewOutputSize;
  readonly onCropPointerDown: (event: PointerEvent<HTMLButtonElement>, edge: CanvasCropEdge) => void;
  readonly onCropKeyDown: (event: KeyboardEvent<HTMLButtonElement>, edge: CanvasCropEdge) => void;
  /** Receives the pointer event when an outside click applies the crop. */
  readonly onCommit: (outsideEvent?: Event) => void;
  readonly onCancel: () => void;
}

/**
 * Crop mode: edge handles at the crop insets and the dashed crop rectangle. Enter or a click outside
 * the controls applies the crop, Escape discards it. The top handle takes focus on entry.
 */
export function CanvasCropControls({ layer, outputSize, onCropPointerDown, onCropKeyDown, onCommit, onCancel }: CropControlsProps) {
  const groupRef = useRef<HTMLDivElement>(null);
  const firstHandleRef = useRef<HTMLButtonElement>(null);
  const hintId = useId();
  const commitRef = useRef(onCommit);
  commitRef.current = onCommit;

  useEffect(() => {
    firstHandleRef.current?.focus();
  }, []);

  useEffect(() => {
    function commitOutside(event: globalThis.PointerEvent) {
      if (event.target instanceof Node && groupRef.current?.contains(event.target)) return;
      commitRef.current(event);
    }
    document.addEventListener("pointerdown", commitOutside, true);
    return () => document.removeEventListener("pointerdown", commitOutside, true);
  }, []);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "Enter" && event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Enter") onCommit();
    else onCancel();
  }

  return (
    <div
      ref={groupRef}
      role="group"
      aria-label={`Canvas crop controls for ${layer.label}`}
      aria-describedby={hintId}
      className="pointer-events-none absolute z-[25] border border-accent/60"
      style={selectionBoxStyle(layer, outputSize)}
      onKeyDown={onKeyDown}
    >
      <span id={hintId} className="sr-only">
        Drag or use arrow keys to crop. Press Enter to apply or Escape to cancel.
      </span>
      <div
        aria-hidden
        className="pointer-events-none absolute border border-dashed border-warning"
        style={{ left: canvasPercent(layer.cropLeft), right: canvasPercent(layer.cropRight), top: canvasPercent(layer.cropTop), bottom: canvasPercent(layer.cropBottom) }}
      />
      {cropEdges.map(({ edge, label, className, style }, index) => (
        <button
          key={edge}
          ref={index === 0 ? firstHandleRef : undefined}
          type="button"
          aria-label={`Crop ${layer.label} from ${label}`}
          className={cn("pointer-events-auto absolute flex h-7 w-7 touch-none items-center justify-center rounded-full text-foreground", handleFocusClassName, className)}
          style={style(layer)}
          onPointerDown={(event) => onCropPointerDown(event, edge)}
          onKeyDown={(event) => onCropKeyDown(event, edge)}
        >
          <span className="flex h-4 w-4 items-center justify-center rounded border border-warning bg-popover/90 shadow-sm">
            <Crop className="h-2.5 w-2.5" aria-hidden />
          </span>
        </button>
      ))}
    </div>
  );
}
