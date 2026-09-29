import { AlignCenter, AlignLeft, AlignRight } from "lucide-react";
import { motionTemplateCatalog } from "@/lib/motion-templates";
import {
  editTextAction,
  textContent,
  textOverlayMetadata,
  textOverlayUpdateAction,
  textStyle,
  textStyleAction,
  type TextStyle,
  type TextStyleEdit,
} from "@/lib/properties/text-properties";
import type { TimelineItem } from "@/lib/timeline";
import { ColorSwatches } from "./controls/color-swatches";
import { PresetGrid } from "./controls/preset-grid";
import { PropertySection } from "./controls/property-section";
import { SegmentedField } from "./controls/segmented-field";
import { SelectField } from "./controls/select-field";
import { SliderField } from "./controls/slider-field";
import { TextCommitField } from "./controls/text-commit-field";
import { formatNumber } from "./formatters";
import type { PropertyTab } from "./property-tabs";
import {
  backgroundColorOptions,
  fontOptions,
  fontSizeRange,
  noColor,
  strokeColorOptions,
  textColorOptions,
} from "./text-style-options";
import { usePropertyCommit } from "./use-property-commit";
import { AnimationPhaseSection } from "./visual-speed-animation";

/** Legacy text treatments: the motion template catalog without the caption templates. */
const textTreatments = motionTemplateCatalog.filter((template) => template.category !== "captions");

const alignmentOptions = [
  { value: "left", label: "Align left", icon: AlignLeft },
  { value: "center", label: "Align center", icon: AlignCenter },
  { value: "right", label: "Align right", icon: AlignRight },
] as const;

function useTextStyle(item: TimelineItem) {
  const { commit, preview } = usePropertyCommit();
  return {
    style: textStyle(item),
    preview,
    apply: (edit: TextStyleEdit) => commit(textStyleAction([item.id], edit)),
  };
}

function TextContentTab({ item }: { readonly item: TimelineItem }) {
  const { style, preview, apply } = useTextStyle(item);
  const swatchValue = (color: string | null) => color ?? noColor;
  const swatchEdit = (key: keyof Pick<TextStyle, "strokeColor" | "backgroundColor">, color: string) =>
    void apply({ [key]: color === noColor ? null : color });
  return (
    <>
      <PropertySection title="Text">
        <TextCommitField key={item.id} label="Text" multiline value={textContent(item)} build={(text) => editTextAction(item, text)} />
      </PropertySection>
      <PropertySection title="Font" onReset={() => void apply({ fontName: null, fontSize: null, color: null })}>
        <SelectField label="Family" value={style.fontName} options={fontOptions(style.fontName)} onChange={(fontName) => void apply({ fontName })} />
        <SliderField
          label="Size"
          value={style.fontSize}
          min={fontSizeRange.min}
          max={fontSizeRange.max}
          step={fontSizeRange.step}
          format={formatNumber}
          onPreview={(fontSize) => preview(item.id, { fontSize })}
          onCommit={(fontSize) => apply({ fontSize })}
        />
        <ColorSwatches label="Color" value={style.color} options={textColorOptions} onChange={(color) => void apply({ color })} />
      </PropertySection>
      <PropertySection title="Stroke and background" onReset={() => void apply({ strokeColor: null, backgroundColor: null })}>
        <ColorSwatches label="Stroke" value={swatchValue(style.strokeColor)} options={strokeColorOptions} onChange={(color) => swatchEdit("strokeColor", color)} />
        <ColorSwatches
          label="Background"
          value={swatchValue(style.backgroundColor)}
          options={backgroundColorOptions}
          onChange={(color) => swatchEdit("backgroundColor", color)}
        />
      </PropertySection>
    </>
  );
}

/**
 * Text treatments carry guidance, not typography: choosing one writes its visual treatment,
 * motion, safe zone and avoid notes to the overlay through `updateTextOverlayItems`.
 */
function TextStyleTab({ item }: { readonly item: TimelineItem }) {
  const { commit } = usePropertyCommit();
  const metadata = textOverlayMetadata(item);
  const selected = textTreatments.find((treatment) => treatment.visualTreatment === metadata.visualTreatment)?.id ?? null;
  return (
    <PropertySection title="Preset">
      <PresetGrid
        label="Text style preset"
        columns={2}
        value={selected}
        options={textTreatments.map((treatment) => ({ value: treatment.id, label: treatment.name }))}
        onChange={(id) => {
          const treatment = textTreatments.find((candidate) => candidate.id === id);
          if (!treatment) return;
          const { visualTreatment, motion, safeZone, avoid } = treatment;
          void commit(textOverlayUpdateAction(item, { visualTreatment, motion, safeZone, avoid }));
        }}
      />
      <p className="text-[11px] text-dim">Presets set the treatment, motion and safe-zone guidance used when the overlay renders.</p>
    </PropertySection>
  );
}

/** Alignment only: the project model has no safe-area snapping property to toggle. */
function TextPositionTab({ item }: { readonly item: TimelineItem }) {
  const { style, apply } = useTextStyle(item);
  return (
    <PropertySection title="Position">
      <SegmentedField label="Alignment" value={style.alignment} options={alignmentOptions} onChange={(alignment) => void apply({ alignment })} />
    </PropertySection>
  );
}

/** Text · Style · Position · Animation (in and out presets as keyframe lanes). */
export function TextTabBody({ tab, item }: { readonly tab: PropertyTab; readonly item: TimelineItem }) {
  switch (tab.id) {
    case "style":
      return <TextStyleTab item={item} />;
    case "position":
      return <TextPositionTab item={item} />;
    case "animation":
      return (
        <>
          <AnimationPhaseSection item={item} phase="in" />
          <AnimationPhaseSection item={item} phase="out" />
        </>
      );
    default:
      return <TextContentTab item={item} />;
  }
}
