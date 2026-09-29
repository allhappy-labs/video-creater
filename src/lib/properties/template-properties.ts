import { motionPresetCatalog, type MotionPresetId } from "@/lib/motion-presets";
import { getMotionTemplate, type MotionTemplateDefinition } from "@/lib/motion-templates";
import type { ProjectActionTemplateOverrideUpdate } from "@/lib/project";
import {
  defaultTemplateStyle,
  getMetadataValue,
  templateFieldsForItem,
  templateStyleForItem,
  type TemplateMetadataKey,
} from "@/lib/templates/template-item";
import { isTemplateTimelineItem, type TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { stringProperty } from "@/lib/timeline-ops/item-properties";

export type TemplateStyleKey = keyof typeof defaultTemplateStyle;

export interface TemplateContent {
  readonly template: MotionTemplateDefinition;
  readonly fields: Record<string, string>;
  readonly style: Record<string, string>;
  readonly metadata: Readonly<Record<TemplateMetadataKey, string>>;
}

const notTemplateMessage = "Select a motion template to edit its fields.";

const styleLabels: Readonly<Record<TemplateStyleKey, string>> = {
  accentColor: "Accent color",
  backgroundColor: "Background color",
  textColor: "Text color",
};

const metadataLabels: Readonly<Record<TemplateMetadataKey, string>> = {
  visualTreatment: "Visual treatment",
  motion: "Motion",
  safeZone: "Safe zone",
  avoid: "Avoid",
};

function templateForItem(item: TimelineItem): MotionTemplateDefinition | null {
  const templateId = isTemplateTimelineItem(item) ? stringProperty(item, "templateId") : null;
  return templateId ? getMotionTemplate(templateId) : null;
}

function metadataForItem(item: TimelineItem, template: MotionTemplateDefinition) {
  return {
    visualTreatment: getMetadataValue(item, template, "visualTreatment"),
    motion: getMetadataValue(item, template, "motion"),
    safeZone: getMetadataValue(item, template, "safeZone"),
    avoid: getMetadataValue(item, template, "avoid"),
  };
}

/** Fields (template defaults overlaid by the item), style and guidance for a catalog template. */
export function templateContent(item: TimelineItem): TemplateContent | null {
  const template = templateForItem(item);
  if (!template) return null;
  return {
    template,
    fields: templateFieldsForItem(item, template.id),
    style: templateStyleForItem(item),
    metadata: metadataForItem(item, template),
  };
}

/** The item's motion preset, else the template's default; null for non-templates. */
export function templateMotionPreset(item: TimelineItem): MotionPresetId | null {
  const template = templateForItem(item);
  if (!template) return null;
  const stored = stringProperty(item, "motionPresetId");
  return motionPresetCatalog.find((preset) => preset.id === stored)?.id ?? template.motionPresetId;
}

/** Legacy `applySelectedTemplateField`: trimmed value merged into the fields; timing unchanged. */
export function templateFieldAction(item: TimelineItem, fieldName: string, value: string): CommandResult {
  const template = templateForItem(item);
  if (!template) return { blocked: notTemplateMessage };
  const field = template.fieldDefinitions.find((definition) => definition.name === fieldName);
  if (!field) return { blocked: `${template.name} has no field named ${fieldName}.` };
  const trimmed = value.trim();
  if (field.required && trimmed.length === 0) return { blocked: `${field.label} can't be empty.` };
  return {
    actions: [
      {
        type: "updateTemplateItems",
        updates: [
          {
            itemId: item.id,
            startSeconds: item.startSeconds,
            durationSeconds: item.durationSeconds,
            templateFields: { ...templateFieldsForItem(item, template.id), [fieldName]: trimmed },
          },
        ],
      },
    ],
  };
}

function overrideAction(
  item: TimelineItem,
  template: MotionTemplateDefinition,
  change: Partial<Pick<ProjectActionTemplateOverrideUpdate, "style" | TemplateMetadataKey>>,
): CommandResult {
  return {
    actions: [
      {
        type: "updateTemplateOverride",
        override: {
          templateId: template.id,
          name: template.name,
          fields: templateFieldsForItem(item, template.id),
          style: templateStyleForItem(item),
          ...metadataForItem(item, template),
          ...change,
        },
      },
    ],
  };
}

/** Legacy `applySelectedTemplateStyle`: one trimmed style key through `updateTemplateOverride`. */
export function templateStyleAction(item: TimelineItem, key: TemplateStyleKey, value: string): CommandResult {
  const template = templateForItem(item);
  if (!template) return { blocked: notTemplateMessage };
  const trimmed = value.trim();
  if (trimmed.length === 0) return { blocked: `${styleLabels[key]} can't be empty.` };
  return overrideAction(item, template, { style: { ...templateStyleForItem(item), [key]: trimmed } });
}

/** Legacy `applySelectedTemplateMetadata`: one trimmed guidance key through `updateTemplateOverride`. */
export function templateMetadataAction(item: TimelineItem, key: TemplateMetadataKey, value: string): CommandResult {
  const template = templateForItem(item);
  if (!template) return { blocked: notTemplateMessage };
  const trimmed = value.trim();
  if (trimmed.length === 0) return { blocked: `${metadataLabels[key]} can't be empty.` };
  return overrideAction(item, template, { [key]: trimmed });
}
