import type { ProjectAction } from "@/lib/project";
import { getTimelineItemText, isTemplateTimelineItem, type TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { numberProperty, stringProperty } from "@/lib/timeline-ops/item-properties";

type TextAlignment = "left" | "center" | "right";

export interface TextStyle {
  readonly fontName: string;
  readonly fontSize: number;
  readonly color: string;
  readonly alignment: TextAlignment;
  /** Stored as `borderColor`, the key the backend text tools write. */
  readonly strokeColor: string | null;
  readonly backgroundColor: string | null;
}

export type TextStyleEdit = { readonly [Key in keyof TextStyle]?: TextStyle[Key] | null };

export interface TextOverlayMetadata {
  readonly visualTreatment: string;
  readonly motion: string;
  readonly safeZone: string;
  readonly avoid: string;
}

export type TextOverlayEdit = Partial<TextOverlayMetadata & { readonly text: string }>;

/** Defaults the backend applies when exporting text (FCPXML text style). */
const defaultTextStyle = {
  fontName: "Helvetica",
  fontSize: 48,
  color: "#ffffff",
  alignment: "center",
} as const satisfies Partial<TextStyle>;

/** Legacy inline text overlay defaults for missing metadata. */
const defaultTextOverlayMetadata: TextOverlayMetadata = {
  visualTreatment: "clean editable text overlay with high-contrast type and transparent backing",
  motion: "quick fade in, hold, and soft fade out",
  safeZone: "keep text inside 10% title-safe margins",
  avoid: "opaque slabs, default-font template look, and covering faces or key action",
};

const overlayFieldLabels = {
  text: "Text",
  visualTreatment: "Visual treatment",
  motion: "Motion",
  safeZone: "Safe zone",
  avoid: "Avoid",
} as const;

const textAlignments: readonly TextAlignment[] = ["left", "center", "right"];

const stylePropertyKeys: Readonly<Record<keyof TextStyle, string>> = {
  fontName: "fontName",
  fontSize: "fontSize",
  color: "color",
  alignment: "alignment",
  strokeColor: "borderColor",
  backgroundColor: "backgroundColor",
};

const styleLabels: Readonly<Record<keyof TextStyle, string>> = {
  fontName: "Font",
  fontSize: "Font size",
  color: "Text color",
  alignment: "Alignment",
  strokeColor: "Stroke color",
  backgroundColor: "Background color",
};

function isTextOverlay(item: TimelineItem) {
  return item.kind === "overlay" && item.source.type === "text" && !isTemplateTimelineItem(item);
}

export function textContent(item: TimelineItem): string {
  return getTimelineItemText(item);
}

export function textStyle(item: TimelineItem): TextStyle {
  const fontSize = numberProperty(item, "fontSize");
  const alignment = stringProperty(item, "alignment");
  return {
    fontName: stringProperty(item, "fontName") ?? defaultTextStyle.fontName,
    fontSize: fontSize !== null && fontSize > 0 ? fontSize : defaultTextStyle.fontSize,
    color: stringProperty(item, "color") ?? defaultTextStyle.color,
    alignment: textAlignments.find((candidate) => candidate === alignment) ?? defaultTextStyle.alignment,
    strokeColor: stringProperty(item, stylePropertyKeys.strokeColor),
    backgroundColor: stringProperty(item, "backgroundColor"),
  };
}

export function textOverlayMetadata(item: TimelineItem): TextOverlayMetadata {
  return {
    visualTreatment: stringProperty(item, "visualTreatment") ?? defaultTextOverlayMetadata.visualTreatment,
    motion: stringProperty(item, "motion") ?? defaultTextOverlayMetadata.motion,
    safeZone: stringProperty(item, "safeZone") ?? defaultTextOverlayMetadata.safeZone,
    avoid: stringProperty(item, "avoid") ?? defaultTextOverlayMetadata.avoid,
  };
}

/** Text content edit for a plain text overlay; the text is trimmed like the legacy inline edit. */
export function editTextAction(item: TimelineItem, text: string): CommandResult {
  if (!isTextOverlay(item)) return { blocked: "Select a text overlay to edit its text." };
  const trimmed = text.trim();
  if (trimmed.length === 0) return { blocked: "Text can't be empty." };
  return { actions: [{ type: "editTextItem", itemId: item.id, text: trimmed }] };
}

/**
 * Text and guidance metadata for a text overlay. Timing is kept as stored (Properties has no
 * timing fields) and unspecified metadata falls back to the legacy inline defaults.
 */
export function textOverlayUpdateAction(item: TimelineItem, edit: TextOverlayEdit): CommandResult {
  if (!isTextOverlay(item)) return { blocked: "Select a text overlay to edit its details." };
  const next = { text: textContent(item), ...textOverlayMetadata(item), ...edit };
  for (const [field, label] of Object.entries(overlayFieldLabels) as [keyof typeof next, string][]) {
    if (next[field].trim().length === 0) return { blocked: `${label} can't be empty.` };
  }
  return {
    actions: [
      {
        type: "updateTextOverlayItems",
        updates: [
          { itemId: item.id, startSeconds: item.startSeconds, durationSeconds: item.durationSeconds, ...next },
        ],
      },
    ],
  };
}

/** Font, size, color, stroke, background and alignment for one or more text items; null resets. */
export function textStyleAction(itemIds: readonly string[], edit: TextStyleEdit): CommandResult {
  if (itemIds.length === 0) return { blocked: "Select a text item to style." };
  const set: Record<string, unknown> = {};
  const remove: string[] = [];
  for (const key of Object.keys(stylePropertyKeys) as (keyof TextStyle)[]) {
    const value = edit[key];
    if (value === undefined) continue;
    const propertyKey = stylePropertyKeys[key];
    if (value === null) {
      remove.push(propertyKey);
    } else if (typeof value === "number") {
      if (!Number.isFinite(value) || value <= 0) return { blocked: `${styleLabels[key]} must be greater than 0.` };
      set[propertyKey] = value;
    } else if (key === "alignment" && !textAlignments.some((alignment) => alignment === value)) {
      return { blocked: "Alignment must be left, center, or right." };
    } else if (value.trim().length === 0) {
      return { blocked: `${styleLabels[key]} can't be empty.` };
    } else {
      set[propertyKey] = value.trim();
    }
  }
  if (Object.keys(set).length === 0 && remove.length === 0) return { blocked: "Change at least one text style." };
  const action: ProjectAction = {
    type: "updateItemProperties",
    updates: itemIds.map((itemId) => ({ itemId, set: { ...set }, remove: [...remove] })),
  };
  return { actions: [action] };
}
