import { getMotionTemplate, type MotionTemplateDefinition } from "@/lib/motion-templates";
import type { ShaderBackgroundTemplateDefinition } from "@/lib/shader-background-templates";
import type { TimelineItem } from "@/lib/timeline";

export const defaultTemplateStyle = {
  accentColor: "#22d3ee",
  backgroundColor: "rgba(2, 6, 23, 0.72)",
  textColor: "#ffffff",
};

export function templateFieldsForItem(item: TimelineItem, templateId: string) {
  const template = getMotionTemplate(templateId);
  const rawFields = item.properties.templateFields;
  const currentFields =
    rawFields && typeof rawFields === "object" && !Array.isArray(rawFields)
      ? Object.fromEntries(
          Object.entries(rawFields).map(([key, fieldValue]) => [
            key,
            String(fieldValue ?? ""),
          ]),
        )
      : {};

  return {
    ...(template?.defaultTextFields ?? {}),
    ...currentFields,
  };
}

export function templateStyleForItem(item: TimelineItem) {
  const rawStyle = item.properties.templateStyle;
  const currentStyle =
    rawStyle && typeof rawStyle === "object" && !Array.isArray(rawStyle)
      ? Object.fromEntries(
          Object.entries(rawStyle).flatMap(([key, styleValue]) =>
            typeof styleValue === "string" && styleValue.trim().length > 0
              ? [[key, styleValue.trim()]]
              : [],
          ),
        )
      : {};

  return {
    ...defaultTemplateStyle,
    ...currentStyle,
  };
}

export function templateMetadataForItem(
  item: TimelineItem,
  template: NonNullable<ReturnType<typeof getMotionTemplate>>,
) {
  return {
    visualTreatment:
      typeof item.properties.visualTreatment === "string"
        ? item.properties.visualTreatment
        : template.visualTreatment,
    motion:
      typeof item.properties.motion === "string" ? item.properties.motion : template.motion,
    safeZone:
      typeof item.properties.safeZone === "string"
        ? item.properties.safeZone
        : template.safeZone,
    avoid:
      typeof item.properties.avoid === "string" ? item.properties.avoid : template.avoid,
  };
}

export function getTemplateFields(item: TimelineItem | null): Record<string, string> {
  const fields = item?.properties.templateFields;
  return fields && typeof fields === "object" && !Array.isArray(fields)
    ? Object.fromEntries(
        Object.entries(fields).map(([key, value]) => [key, String(value ?? "")]),
      )
    : {};
}

export type TemplateMetadataKey = "visualTreatment" | "motion" | "safeZone" | "avoid";

export function getStyleValue(item: TimelineItem | null, key: keyof typeof defaultTemplateStyle): string {
  const style = item?.properties.templateStyle;
  if (style && typeof style === "object" && !Array.isArray(style)) {
    const value = (style as Record<string, unknown>)[key];
    if (typeof value === "string" && value.trim().length > 0) {
      return value;
    }
  }

  return defaultTemplateStyle[key];
}

export function getMetadataValue(
  item: TimelineItem | null,
  template: MotionTemplateDefinition | null,
  key: TemplateMetadataKey,
): string {
  const value = item?.properties[key];
  if (typeof value === "string" && value.trim().length > 0) {
    return value;
  }

  return template?.[key] ?? "";
}

export function formatCategory(category: string) {
  const label = category.split("_").join(" ");
  return label.charAt(0).toUpperCase() + label.slice(1);
}

export function formatSourceKind(sourceKind: ShaderBackgroundTemplateDefinition["sourceKind"]) {
  return sourceKind === "user" ? "User" : "Built-in";
}

export function formatTrack(trackKind: MotionTemplateDefinition["placement"]["trackKind"]) {
  if (trackKind === "overlay") {
    return "Overlays";
  }
  if (trackKind === "caption") {
    return "Captions";
  }
  if (trackKind === "hyperframe_scene") {
    return "HyperFrames";
  }
  return trackKind;
}

export function formatPlacementTime(seconds: number) {
  const safeSeconds = Math.max(0, seconds);
  const minutes = Math.floor(safeSeconds / 60);
  const remainingSeconds = (safeSeconds % 60).toFixed(3).padStart(6, "0");
  return `${minutes.toString().padStart(2, "0")}:${remainingSeconds}`;
}

export type TemplateCategoryFilter =
  | "overlays"
  | "captions"
  | "hyperframes"
  | "transitions"
  | "shader-backgrounds";

export const categoryLabels: Record<TemplateCategoryFilter, string> = {
  overlays: "Overlays",
  captions: "Captions",
  hyperframes: "HyperFrames",
  transitions: "Transitions",
  "shader-backgrounds": "Shader backgrounds",
};

export const categoryOrder: readonly TemplateCategoryFilter[] = [
  "overlays",
  "captions",
  "hyperframes",
  "transitions",
  "shader-backgrounds",
];

export function templateCategories(template: MotionTemplateDefinition): readonly TemplateCategoryFilter[] {
  if (template.kind === "transition" || template.category === "transitions") {
    return ["transitions"];
  }
  if (
    template.kind === "hyperframe_scene" ||
    template.placement.trackKind === "hyperframe_scene"
  ) {
    return ["hyperframes"];
  }
  if (
    template.kind === "caption" ||
    template.category === "captions" ||
    template.placement.trackKind === "caption"
  ) {
    return ["captions"];
  }
  return ["overlays"];
}

export function backgroundCategories(): readonly TemplateCategoryFilter[] {
  return ["hyperframes", "shader-backgrounds"];
}

export const gradientLoopPanelStyles = [
  "linear-gradient(180deg,#146492 0%,#0b3b56 25%,#d4e1e1 31%,#d6e3e0 50%,#2191d6 62%,#1c87d1 100%)",
  "linear-gradient(180deg,#0d4363 0%,#031417 22%,#0a405f 46%,#bddfe9 58%,#75b9d6 78%,#1f8cd0 100%)",
  "linear-gradient(180deg,#40aaf0 0%,#3da3dc 5%,#020f12 6%,#031f2b 36%,#104d73 56%,#031416 70%,#1f92d8 100%)",
  "linear-gradient(180deg,#1f8bd0 0%,#166b9e 22%,#0f496b 54%,#1e8bd1 68%,#2094dc 100%)",
  "linear-gradient(180deg,#aad5e3 0%,#6bb3d6 16%,#2393d6 48%,#2aa1e4 74%,#d5e2df 80%,#d6e0de 100%)",
] as const;

export function splitPreviewLines(value: string | undefined): string[] {
  const lines = (value ?? "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  return lines.length > 0 ? lines : [""];
}
