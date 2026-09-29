import { Crop } from "lucide-react";
import type { AnimatableProperty } from "@/lib/properties/keyframe-actions";
import {
  blendModeAction,
  cropActions,
  fadesAction,
  motionActions,
  opacityActions,
  visualBlendMode,
  visualCrop,
  visualFades,
  visualMotion,
  visualOpacity,
  visualPropertyRanges,
  type VisualCrop,
} from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { VisualBlendMode } from "@/lib/timeline-ops/item-properties";
import type { VisualMotionKeyframeProperty } from "@/lib/timeline-ops/keyframes";
import { cropModePlayheadSeconds } from "../preview/canvas-editing";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { PropertySection } from "./controls/property-section";
import { SelectField } from "./controls/select-field";
import { SliderField } from "./controls/slider-field";
import { formatDegrees, formatNumber, formatPercent, formatSeconds, fromPercent, toPercent } from "./formatters";
import { useAnimatedProperty } from "./use-animated-property";
import { combineResults, usePropertyCommit } from "./use-property-commit";
import { EffectsSection, LookSection } from "./visual-look-effects";

interface MotionField {
  readonly property: VisualMotionKeyframeProperty;
  readonly label: string;
  /** Slider units; scale is shown in percent. */
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly format: (value: number) => string;
  readonly toDisplay: (value: number) => number;
  readonly fromDisplay: (value: number) => number;
}

const same = (value: number) => value;

/**
 * Slider ranges inside the backend motion bounds (scale 0.01–100, position ±10000,
 * rotation ±360): scale 1–500% and position ±2000 px keep the sliders usable.
 */
const motionFields: readonly MotionField[] = [
  { property: "scale", label: "Scale", min: 1, max: 500, step: 1, format: formatPercent, toDisplay: toPercent, fromDisplay: fromPercent },
  { property: "positionX", label: "Position X", min: -2000, max: 2000, step: 1, format: formatNumber, toDisplay: same, fromDisplay: same },
  { property: "positionY", label: "Position Y", min: -2000, max: 2000, step: 1, format: formatNumber, toDisplay: same, fromDisplay: same },
  { property: "rotationDegrees", label: "Rotate", min: -360, max: 360, step: 1, format: formatDegrees, toDisplay: same, fromDisplay: same },
];

const motionDefaults: Readonly<Record<VisualMotionKeyframeProperty, number>> = { scale: 1, positionX: 0, positionY: 0, rotationDegrees: 0 };

const blendModeOptions: readonly { readonly value: VisualBlendMode; readonly label: string }[] = [
  { value: "over", label: "Normal" },
  { value: "darken", label: "Darken" },
  { value: "multiply", label: "Multiply" },
  { value: "colorBurn", label: "Color Burn" },
  { value: "lighten", label: "Lighten" },
  { value: "screen", label: "Screen" },
  { value: "colorDodge", label: "Color Dodge" },
  { value: "overlay", label: "Overlay" },
  { value: "softLight", label: "Soft Light" },
  { value: "hardLight", label: "Hard Light" },
  { value: "difference", label: "Difference" },
  { value: "exclusion", label: "Exclusion" },
  { value: "hue", label: "Hue" },
  { value: "saturation", label: "Saturation" },
  { value: "color", label: "Color" },
  { value: "luminosity", label: "Luminosity" },
  { value: "add", label: "Add" },
];

const cropSides: readonly { readonly side: keyof VisualCrop & AnimatableProperty; readonly label: string }[] = [
  { side: "cropTop", label: "Top" },
  { side: "cropRight", label: "Right" },
  { side: "cropBottom", label: "Bottom" },
  { side: "cropLeft", label: "Left" },
];

function MotionSlider({ item, field }: { readonly item: TimelineItem; readonly field: MotionField }) {
  const { commit, preview } = usePropertyCommit();
  const animated = useAnimatedProperty(item, field.property, visualMotion(item)[field.property]);
  return (
    <SliderField
      label={field.label}
      value={field.toDisplay(animated.value)}
      min={field.min}
      max={field.max}
      step={field.step}
      format={field.format}
      keyframe={animated.keyframe}
      onPreview={(value) => preview(item.id, { [field.property]: field.fromDisplay(value) })}
      onCommit={(value) => commit(motionActions(item, field.property, field.fromDisplay(value), animated.playheadSeconds))}
    />
  );
}

function OpacitySlider({ item }: { readonly item: TimelineItem }) {
  const { commit, preview } = usePropertyCommit();
  const animated = useAnimatedProperty(item, "opacity", visualOpacity(item));
  const range = visualPropertyRanges.opacity;
  return (
    <SliderField
      label="Opacity"
      value={toPercent(animated.value)}
      min={toPercent(range.min)}
      max={toPercent(range.max)}
      step={toPercent(range.step)}
      format={formatPercent}
      keyframe={animated.keyframe}
      onPreview={(value) => preview(item.id, { opacity: fromPercent(value) })}
      onCommit={(value) => commit(opacityActions(item, fromPercent(value), animated.playheadSeconds))}
    />
  );
}

export function TransformSection({ item }: { readonly item: TimelineItem }) {
  const { commit } = usePropertyCommit();
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const reset = () =>
    commit(
      combineResults([
        ...motionFields.map((field) => motionActions(item, field.property, motionDefaults[field.property], playheadSeconds)),
        opacityActions(item, 1, playheadSeconds),
      ]),
    );
  return (
    <PropertySection title="Transform" onReset={reset}>
      {motionFields.map((field) => (
        <MotionSlider key={field.property} item={item} field={field} />
      ))}
      <OpacitySlider item={item} />
    </PropertySection>
  );
}

export function BlendSection({ item }: { readonly item: TimelineItem }) {
  const { commit } = usePropertyCommit();
  return (
    <PropertySection title="Blend">
      <SelectField
        label="Blend mode"
        value={visualBlendMode(item)}
        options={blendModeOptions}
        onChange={(mode) => void commit([blendModeAction(item.id, mode)])}
      />
    </PropertySection>
  );
}

function CropSlider({ item, side, label }: { readonly item: TimelineItem; readonly side: keyof VisualCrop & AnimatableProperty; readonly label: string }) {
  const { commit, preview } = usePropertyCommit();
  const animated = useAnimatedProperty(item, side, visualCrop(item)[side]);
  const range = visualPropertyRanges.crop;
  return (
    <SliderField
      label={`Crop ${label.toLowerCase()}`}
      value={toPercent(animated.value)}
      min={toPercent(range.min)}
      max={toPercent(range.max)}
      step={toPercent(range.step)}
      format={formatPercent}
      keyframe={animated.keyframe}
      onPreview={(value) => preview(item.id, { [side]: fromPercent(value) })}
      onCommit={(value) => commit(cropActions(item, { [side]: fromPercent(value) }, animated.playheadSeconds))}
    />
  );
}

export function CropSection({ item }: { readonly item: TimelineItem }) {
  const { commit } = usePropertyCommit();
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const store = useEditorStoreApi();
  /** Pauses and, when the playhead is outside the clip, seeks into it so the canvas has its layer to crop. */
  const enterCropMode = () => {
    const state = store.getState();
    const seconds = cropModePlayheadSeconds(item, state.playheadSeconds, state.project.renderSettings.fps);
    state.setPlaying(false);
    if (seconds !== null) state.seek(seconds);
    state.setCropModeItemId(item.id);
  };
  const editOnCanvas = (
    <button
      type="button"
      onClick={enterCropMode}
      className="flex h-7 items-center gap-1.5 rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <Crop className="h-3.5 w-3.5" aria-hidden />
      Edit on canvas
    </button>
  );
  const reset = () => commit(cropActions(item, { cropTop: 0, cropRight: 0, cropBottom: 0, cropLeft: 0 }, playheadSeconds));
  return (
    <PropertySection title="Crop" action={editOnCanvas} onReset={reset}>
      {cropSides.map(({ side, label }) => (
        <CropSlider key={side} item={item} side={side} label={label} />
      ))}
    </PropertySection>
  );
}

export function FadeSection({ item }: { readonly item: TimelineItem }) {
  const { commit, preview } = usePropertyCommit();
  const fades = visualFades(item);
  const max = Math.max(item.durationSeconds, 0);
  const step = visualPropertyRanges.fade.step;
  return (
    <PropertySection title="Fade" onReset={() => commit(fadesAction(item, 0, 0))}>
      <SliderField
        label="Fade in"
        value={Math.min(fades.fadeInSeconds, max)}
        min={0}
        max={max}
        step={step}
        format={formatSeconds}
        onPreview={(value) => preview(item.id, { fadeInSeconds: value })}
        onCommit={(value) => commit(fadesAction(item, value, fades.fadeOutSeconds))}
      />
      <SliderField
        label="Fade out"
        value={Math.min(fades.fadeOutSeconds, max)}
        min={0}
        max={max}
        step={step}
        format={formatSeconds}
        onPreview={(value) => preview(item.id, { fadeOutSeconds: value })}
        onCommit={(value) => commit(fadesAction(item, fades.fadeInSeconds, value))}
      />
    </PropertySection>
  );
}

/** Video tab: transform, blend, crop, fades, look and effects. No timing fields. */
export function VisualVideoTab({ item }: { readonly item: TimelineItem }) {
  return (
    <>
      <TransformSection item={item} />
      <BlendSection item={item} />
      <CropSection item={item} />
      <FadeSection item={item} />
      <LookSection item={item} />
      <EffectsSection items={[item]} />
    </>
  );
}
