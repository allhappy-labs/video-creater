import type { CSSProperties, ReactNode } from "react";
import { previewMotionStyle, type PreviewOutputSize } from "@/lib/preview/canvas-geometry";
import type { TimelineItem } from "@/lib/timeline";
import type { TimelinePreviewFrame } from "@/lib/timeline-preview";
import { cn } from "@/lib/utils";
import { CaptionText } from "./caption-text";
import { InlineTextEditor, type OverlayTextInteraction } from "./inline-text-editor";
import { TemplateOverlay } from "./template-overlay";

type TimelinePreviewOverlayLayer = TimelinePreviewFrame["overlayLayers"][number];

const captionPlacementClassName = {
  upper: "top-[12%]",
  center: "top-[42%]",
  lower: "bottom-[12%]",
} as const;

// Sizes use container query units so text keeps its proportion to the canvas at every preview size.
const captionPresetClassName = {
  boldReadableLower: "rounded-sm bg-background/55 px-[0.6em] py-[0.25em] text-[length:3.4cqw] font-bold leading-tight shadow-lg",
  kineticFocus: "rounded-sm bg-keyframe/15 px-[0.6em] py-[0.25em] text-[length:3.9cqw] font-black tracking-tight shadow-lg",
  centeredMinimal: "rounded-xl bg-background/55 px-[0.8em] py-[0.35em] text-[length:3cqw] font-semibold tracking-wide shadow-xl",
} as const;

/** Motion without opacity on the positioned wrapper; opacity applies to the visible box only. */
function wrapperStyle(layer: TimelinePreviewOverlayLayer, outputSize: PreviewOutputSize): CSSProperties {
  return previewMotionStyle({ ...layer, opacity: 1 }, outputSize);
}

/**
 * Pointer handlers and class for the visible text box when the layer takes part in canvas selection:
 * a press selects it, a double-click starts inline editing when its track allows it.
 */
function textBoxInteraction(layer: TimelinePreviewOverlayLayer, interaction: OverlayTextInteraction | undefined) {
  if (!interaction) return { className: undefined, handlers: {} };
  const editable = interaction.editableItemIds.has(layer.itemId);
  return {
    className: "pointer-events-auto cursor-default",
    handlers: {
      onPointerDown: () => interaction.onSelect(layer.itemId),
      onDoubleClick: editable ? () => interaction.onStartEdit(layer.itemId) : undefined,
    },
  };
}

function textContent(layer: TimelinePreviewOverlayLayer, text: string, interaction: OverlayTextInteraction | undefined, children: ReactNode) {
  if (!interaction || interaction.editingItemId !== layer.itemId || !interaction.editableItemIds.has(layer.itemId)) return children;
  return (
    <InlineTextEditor
      label={`Edit ${layer.overlayKind === "caption" ? "caption" : "text"} ${layer.label}`}
      text={text}
      onCommit={(next) => interaction.onCommit(layer.itemId, next)}
      onCancel={interaction.onCancel}
    />
  );
}

/** A caption, text overlay or motion template drawn over the media layers. */
export function PreviewOverlayLayer({
  layer,
  item,
  outputSize,
  interaction,
}: {
  readonly layer: TimelinePreviewOverlayLayer;
  /** Render size for position offsets. */
  readonly outputSize: PreviewOutputSize;
  /** The timeline item behind a template layer, or null when it is not on the edited timeline. */
  readonly item: TimelineItem | null;
  /** Canvas selection and inline editing; the layer is display-only without it. */
  readonly interaction?: OverlayTextInteraction | undefined;
}) {
  if (layer.overlayKind === "template") {
    return item ? <TemplateOverlay item={item} style={previewMotionStyle(layer, outputSize)} /> : null;
  }
  if (!layer.text) return null;
  const box = textBoxInteraction(layer, interaction);

  if (layer.overlayKind === "caption") {
    return (
      <div
        aria-label={`Timeline preview caption ${layer.label}`}
        data-caption-style={layer.captionStylePreset ?? "boldReadableLower"}
        className={cn("pointer-events-none absolute inset-x-[16%] z-20 text-center text-foreground", captionPlacementClassName[layer.captionPlacement ?? "lower"])}
        style={wrapperStyle(layer, outputSize)}
      >
        <span className={cn(captionPresetClassName[layer.captionStylePreset ?? "boldReadableLower"], box.className)} style={{ opacity: layer.opacity }} {...box.handlers}>
          {textContent(
            layer,
            layer.text,
            interaction,
            <CaptionText
              text={layer.text}
              emphasizedWordIndices={layer.activeEmphasizedWordIndices ?? layer.emphasizedWordIndices}
              wordStyles={layer.captionWordStyles}
            />,
          )}
        </span>
      </div>
    );
  }

  return (
    <div
      aria-label={`Timeline preview text ${layer.label}`}
      className="pointer-events-none absolute left-[8%] top-[12%] z-20 max-w-[46%] text-foreground"
      style={wrapperStyle(layer, outputSize)}
    >
      <span
        className={cn(
          "block rounded-sm border border-foreground/20 bg-background/45 px-[0.7em] py-[0.45em] text-[length:2.6cqw] font-semibold leading-tight shadow-lg backdrop-blur-sm",
          box.className,
        )}
        style={{ opacity: layer.opacity }}
        {...box.handlers}
      >
        {textContent(layer, layer.text, interaction, layer.text)}
      </span>
    </div>
  );
}
