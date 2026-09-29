import type { MotionPresetId } from "@/lib/motion-presets";
import { motionPresetCatalog } from "@/lib/motion-presets";
import { motionPresetAction } from "@/lib/properties/animation-presets";
import {
  templateContent,
  templateFieldAction,
  templateMetadataAction,
  templateMotionPreset,
  templateStyleAction,
  type TemplateContent,
  type TemplateStyleKey,
} from "@/lib/properties/template-properties";
import { defaultTemplateStyle, type TemplateMetadataKey } from "@/lib/templates/template-item";
import type { TimelineItem } from "@/lib/timeline";
import { PresetGrid } from "./controls/preset-grid";
import { PropertySection } from "./controls/property-section";
import { TextCommitField } from "./controls/text-commit-field";
import type { PropertyTab } from "./property-tabs";
import { usePropertyCommit } from "./use-property-commit";
import { EffectsSection } from "./visual-look-effects";

const styleFields: readonly { readonly key: TemplateStyleKey; readonly label: string }[] = [
  { key: "accentColor", label: "Accent color" },
  { key: "backgroundColor", label: "Background color" },
  { key: "textColor", label: "Text color" },
];

const guidanceFields: readonly { readonly key: TemplateMetadataKey; readonly label: string }[] = [
  { key: "visualTreatment", label: "Visual treatment" },
  { key: "motion", label: "Motion" },
  { key: "safeZone", label: "Safe zone" },
  { key: "avoid", label: "Avoid" },
];

const motionOptions = motionPresetCatalog.map((preset) => ({ value: preset.id, label: preset.label }));

function MissingTemplate() {
  return <p className="px-3 py-6 text-center text-[12px] text-dim">Select a motion template to edit its fields.</p>;
}

/** Template fields; required fields reject empty text inline. */
function TemplateContentTab({ item, content }: { readonly item: TimelineItem; readonly content: TemplateContent }) {
  return (
    <PropertySection title="Fields">
      {content.template.fieldDefinitions.map((field) => (
        <TextCommitField
          key={`${item.id}-${field.name}`}
          label={field.label}
          required={field.required}
          multiline={field.multiline}
          value={content.fields[field.name] ?? ""}
          build={(text) => templateFieldAction(item, field.name, text)}
        />
      ))}
    </PropertySection>
  );
}

/** Style colors (free text, as stored) and the render guidance, each via `updateTemplateOverride`. */
function TemplateStyleTab({ item, content }: { readonly item: TimelineItem; readonly content: TemplateContent }) {
  return (
    <>
      <PropertySection title="Colors">
        {styleFields.map(({ key, label }) => (
          <TextCommitField
            key={`${item.id}-${key}`}
            layout="row"
            required
            label={label}
            value={content.style[key] ?? defaultTemplateStyle[key]}
            build={(text) => templateStyleAction(item, key, text)}
          />
        ))}
      </PropertySection>
      <PropertySection title="Guidance">
        {guidanceFields.map(({ key, label }) => (
          <TextCommitField
            key={`${item.id}-${key}`}
            multiline
            rows={2}
            required
            label={label}
            value={content.metadata[key]}
            build={(text) => templateMetadataAction(item, key, text)}
          />
        ))}
      </PropertySection>
    </>
  );
}

function TemplateAnimationTab({ item }: { readonly item: TimelineItem }) {
  const { commit } = usePropertyCommit();
  return (
    <PropertySection title="Motion">
      <PresetGrid<MotionPresetId>
        label="Template motion"
        columns={2}
        value={templateMotionPreset(item)}
        options={motionOptions}
        onChange={(presetId) => void commit([motionPresetAction([item.id], presetId)])}
      />
    </PropertySection>
  );
}

/** Content · Style · Animation · Effects for a catalog motion template. */
export function TemplateTabBody({ tab, item }: { readonly tab: PropertyTab; readonly item: TimelineItem }) {
  const content = templateContent(item);
  if (tab.id === "effects") return <EffectsSection items={[item]} />;
  if (!content) return <MissingTemplate />;
  switch (tab.id) {
    case "style":
      return <TemplateStyleTab item={item} content={content} />;
    case "animation":
      return <TemplateAnimationTab item={item} />;
    default:
      return <TemplateContentTab item={item} content={content} />;
  }
}
