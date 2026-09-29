import type { TimelineItem, TrackKind } from "./timeline";
import type { MotionPresetId } from "./motion-presets";
import { builtInVPhotoLogoAssetId } from "./builtin-logo-paths";

type MotionTemplateCategory =
  | "text"
  | "titles"
  | "lower_thirds"
  | "captions"
  | "callouts"
  | "transitions";

type MotionTemplateKind = "overlay" | "caption" | "hyperframe_scene" | "transition";
export type MotionTemplatePreviewVariant =
  | "lower-third"
  | "punchy-caption"
  | "metric-callout"
  | "chapter-card"
  | "tracking-highlight"
  | "holographic-logo"
  | "gradient-background-loop";

interface MotionTemplateFieldDefinition {
  name: string;
  label: string;
  multiline: boolean;
  required: boolean;
}

export interface MotionTemplateDefinition {
  id: string;
  version: number;
  name: string;
  category: MotionTemplateCategory;
  kind: MotionTemplateKind;
  durationSeconds: number;
  fieldDefinitions: MotionTemplateFieldDefinition[];
  defaultTextFields: Record<string, string>;
  preview: {
    thumbnailKind: "css";
    cssVariant: MotionTemplatePreviewVariant;
    description: string;
  };
  placement: {
    trackKind: TrackKind;
    defaultStartSeconds: number;
  };
  renderContract: {
    dimensions: "project";
    fps: "project";
    alpha: boolean;
  };
  motionPresetId: MotionPresetId;
  visualTreatment: string;
  motion: string;
  safeZone: string;
  avoid: string;
}

export interface CreateTemplateOverlayItemInput {
  templateId: string;
  itemId: string;
  startSeconds: number;
  fields?: Record<string, string>;
}

export const kineticLowerThirdTemplate: MotionTemplateDefinition = {
  id: "kinetic-lower-third-v1",
  version: 1,
  name: "Kinetic Lower Third",
  category: "lower_thirds",
  kind: "overlay",
  durationSeconds: 2.4,
  fieldDefinitions: [
    { name: "headline", label: "Headline", multiline: false, required: true },
    { name: "subline", label: "Subline", multiline: false, required: true },
  ],
  defaultTextFields: {
    headline: "Name / Role",
    subline: "Context label",
  },
  preview: {
    thumbnailKind: "css",
    cssVariant: "lower-third",
    description: "Translucent lower-third label with an accent rule and quick kinetic entry.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  motionPresetId: "slide-fade-up-v1",
  visualTreatment:
    "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
  motion: "slide-and-fade in over 8 frames, hold, then soft fade out",
  safeZone: "keep essential text inside 10% margins and below face/action priority areas",
  avoid:
    "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds",
};

const punchyCaptionTemplate: MotionTemplateDefinition = {
  id: "punchy-caption-v1",
  version: 1,
  name: "Punchy Caption",
  category: "captions",
  kind: "overlay",
  durationSeconds: 1.8,
  fieldDefinitions: [
    { name: "headline", label: "Headline", multiline: true, required: true },
    { name: "subline", label: "Subline", multiline: false, required: false },
  ],
  defaultTextFields: {
    headline: "BIG POINT",
    subline: "Quick emphasis",
  },
  preview: {
    thumbnailKind: "css",
    cssVariant: "punchy-caption",
    description: "Bold stacked caption beat with a fast scale pop for short-form emphasis.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0.4,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  motionPresetId: "snap-pop-v1",
  visualTreatment: "large phone-readable caption lockup with accent underline and soft backing",
  motion: "scale pop in over 5 frames, underline wipe, then snap fade out",
  safeZone: "keep caption block inside 10% margins and above bottom controls",
  avoid: "subtitle slabs, tiny type, centered static paragraphs, and covering faces",
};

const metricCalloutTemplate: MotionTemplateDefinition = {
  id: "metric-callout-v1",
  version: 1,
  name: "Metric Callout",
  category: "callouts",
  kind: "overlay",
  durationSeconds: 2.2,
  fieldDefinitions: [
    { name: "headline", label: "Headline", multiline: false, required: true },
    { name: "subline", label: "Subline", multiline: false, required: true },
  ],
  defaultTextFields: {
    headline: "42%",
    subline: "faster workflow",
  },
  preview: {
    thumbnailKind: "css",
    cssVariant: "metric-callout",
    description: "Compact metric tile with an animated count feel and directional accent.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0.6,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  motionPresetId: "metric-count-pop-v1",
  visualTreatment: "floating metric tile with high-contrast number, caption, and directional accent",
  motion: "count-up feel, accent sweep, hold, then slide out",
  safeZone: "keep tile inside 10% margins and away from lower captions",
  avoid: "spreadsheet-like boxes, decorative-only badges, and unreadable dense labels",
};

const chapterCardTemplate: MotionTemplateDefinition = {
  id: "chapter-card-v1",
  version: 1,
  name: "Chapter Card",
  category: "titles",
  kind: "overlay",
  durationSeconds: 2.6,
  fieldDefinitions: [
    { name: "headline", label: "Headline", multiline: false, required: true },
    { name: "subline", label: "Subline", multiline: false, required: true },
  ],
  defaultTextFields: {
    headline: "Chapter 01",
    subline: "The setup",
  },
  preview: {
    thumbnailKind: "css",
    cssVariant: "chapter-card",
    description: "Partial-frame chapter reset with strong type and a vertical reveal line.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  motionPresetId: "vertical-reveal-v1",
  visualTreatment: "left-weighted chapter marker with translucent panel and vertical reveal line",
  motion: "vertical line wipe, text type-on, short hold, then mask out",
  safeZone: "keep text inside 10% margins and leave center action visible",
  avoid: "full-frame static slides, plain centered text, and long title holds",
};

const trackingHighlightTemplate: MotionTemplateDefinition = {
  id: "tracking-highlight-v1",
  version: 1,
  name: "Tracking Highlight",
  category: "callouts",
  kind: "overlay",
  durationSeconds: 1.6,
  fieldDefinitions: [
    { name: "headline", label: "Headline", multiline: false, required: true },
    { name: "subline", label: "Subline", multiline: false, required: false },
  ],
  defaultTextFields: {
    headline: "Watch this",
    subline: "key detail",
  },
  preview: {
    thumbnailKind: "css",
    cssVariant: "tracking-highlight",
    description: "Targeted highlight ring and label for calling out a visual detail.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 1,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  motionPresetId: "tracking-draw-v1",
  visualTreatment: "thin tracking ring with compact label and pointer line",
  motion: "ring draws on, label slides from pointer, then both fade",
  safeZone: "keep label inside 10% margins while pointer can track the visual target",
  avoid: "large opaque callout boxes, covering hands or product details, and static arrows",
};

export const holographicLogoTemplate: MotionTemplateDefinition = {
  id: "holographic-logo-cutout-v1",
  version: 1,
  name: "Holographic Logo Cutout",
  category: "titles",
  kind: "overlay",
  durationSeconds: 3.2,
  fieldDefinitions: [
    { name: "logoAssetId", label: "Logo asset", multiline: false, required: true },
  ],
  defaultTextFields: {
    logoAssetId: builtInVPhotoLogoAssetId,
  },
  preview: {
    thumbnailKind: "css",
    cssVariant: "holographic-logo",
    description:
      "Animated holographic logo cutout with pearlescent shader bands over a dark gradient field.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  motionPresetId: "pulse-emphasis-v2",
  visualTreatment:
    "full-frame dark gradient background with a crisp holographic logo cutout, pearlescent shader bands, and fine controlled grain",
  motion:
    "shader shimmer drifts through the logo mask with a clean first-frame hold and no bevel, fake depth, or rectangular light bars",
  safeZone: "keep the logo inside the central 80% safe zone with no essential detail near edges",
  avoid:
    "plain boxes, static text-only cards, default-font logo substitutes, opaque caption slabs, harsh strobes, and long unmoving holds",
};

export const gradientBackgroundLoopTemplate: MotionTemplateDefinition = {
  id: "gradient-background-loop-v1",
  version: 1,
  name: "Gradient Background Loop",
  category: "text",
  kind: "overlay",
  durationSeconds: 4,
  fieldDefinitions: [
    { name: "headline", label: "Headline", multiline: true, required: true },
  ],
  defaultTextFields: {
    headline: "Love\nwins.",
  },
  preview: {
    thumbnailKind: "css",
    cssVariant: "gradient-background-loop",
    description:
      "Oversized white text over looping vertical blue gradient panels, inspired by editorial social title cards.",
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0,
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true,
  },
  motionPresetId: "soft-depth-card-v2",
  visualTreatment:
    "full-frame vertical blue gradient panels with oversized bold white text centered across the columns",
  motion:
    "seamless loop with subtle vertical panel drift, breathing blue gradients, and a steady high-contrast text hold",
  safeZone: "keep headline inside 10% margins while allowing the panel background to fill frame",
  avoid:
    "plain boxes, opaque caption slabs, tiny text, default-font title cards, hard cuts between panel colors, and strobing",
};

export const motionTemplateCatalog: MotionTemplateDefinition[] = [
  kineticLowerThirdTemplate,
  punchyCaptionTemplate,
  metricCalloutTemplate,
  chapterCardTemplate,
  trackingHighlightTemplate,
  holographicLogoTemplate,
  gradientBackgroundLoopTemplate,
];

export function getMotionTemplate(templateId: string): MotionTemplateDefinition | null {
  return motionTemplateCatalog.find((template) => template.id === templateId) ?? null;
}

export function createTemplateOverlayItem(input: CreateTemplateOverlayItemInput): TimelineItem {
  const template = getMotionTemplate(input.templateId);
  if (!template) {
    throw new Error(`Unknown motion template: ${input.templateId}`);
  }
  if (template.kind !== "overlay" || template.placement.trackKind !== "overlay") {
    throw new Error(`Motion template is not an overlay: ${input.templateId}`);
  }
  if (!Number.isFinite(input.startSeconds) || input.startSeconds < 0) {
    throw new Error("Template startSeconds must be a finite non-negative number");
  }

  const mergedFields = {
    ...template.defaultTextFields,
    ...(input.fields ?? {}),
  };

  for (const field of template.fieldDefinitions) {
    const fieldName = field.name;
    if (!mergedFields[fieldName]?.trim()) {
      if (!field.required) {
        mergedFields[fieldName] = "";
        continue;
      }
      throw new Error(`Template field ${fieldName} cannot be empty`);
    }
    mergedFields[fieldName] = mergedFields[fieldName].trim();
  }

  return {
    id: input.itemId,
    kind: "overlay",
    startSeconds: input.startSeconds,
    durationSeconds: template.durationSeconds,
    source: {
      type: "generated",
      artifactId: `template:${template.id}:${input.itemId}`,
    },
    label: template.name,
    properties: {
      templateId: template.id,
      templateFields: mergedFields,
      templateCategory: template.category,
      templateVersion: template.version,
      previewVariant: template.preview.cssVariant,
      motionPresetId: template.motionPresetId,
      renderContract: template.renderContract,
      visualTreatment: template.visualTreatment,
      motion: template.motion,
      safeZone: template.safeZone,
      avoid: template.avoid,
    },
  };
}
