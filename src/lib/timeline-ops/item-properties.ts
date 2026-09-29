import {
  legacyEffectInstanceId,
  type ProjectActionEffect,
  type ProjectJobStatus,
  type VideoProject,
  type VisualEffectDescriptor,
} from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { formatSecondsShort as formatSeconds } from "@/lib/format";

export interface GeneratedTimelineWorkflowStatus {
  status: ProjectJobStatus;
  updatedAt: string;
  workflowType: string | null;
  taskQueue: string | null;
}

const visualBlendModes = [
  "over",
  "darken",
  "multiply",
  "colorBurn",
  "lighten",
  "screen",
  "colorDodge",
  "overlay",
  "softLight",
  "hardLight",
  "difference",
  "exclusion",
  "hue",
  "saturation",
  "color",
  "luminosity",
  "add",
] as const;
export type VisualBlendMode = (typeof visualBlendModes)[number];

export function stringProperty(item: TimelineItem, key: string): string | null {
  const value = item.properties[key];
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

export function numberProperty(item: TimelineItem, key: string): number | null {
  const value = item.properties[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export function stringPropertyOrFallback(item: TimelineItem | null, key: string, fallback = "") {
  const value = item?.properties[key];
  return typeof value === "string" ? value : fallback;
}

export function timelineItemSourceMediaId(item: TimelineItem) {
  if (item.source.type === "media") {
    return item.source.mediaId;
  }

  if (item.source.type === "generated") {
    return item.source.artifactId;
  }

  return null;
}

export function projectActionEffectsForItem(item: TimelineItem): ProjectActionEffect[] {
  if (!Array.isArray(item.properties.effects)) return [];

  const occurrences = new Map<string, number>();
  return item.properties.effects.flatMap((effect): ProjectActionEffect[] => {
    if (!effect || typeof effect !== "object" || Array.isArray(effect)) return [];
    const record = effect as Record<string, unknown>;
    if (
      typeof record.effectType !== "string" ||
      typeof record.enabled !== "boolean" ||
      !record.params ||
      typeof record.params !== "object" ||
      Array.isArray(record.params)
    ) {
      return [];
    }

    const occurrence = occurrences.get(record.effectType) ?? 0;
    occurrences.set(record.effectType, occurrence + 1);
    return [{
      effectInstanceId:
        typeof record.effectInstanceId === "string" && record.effectInstanceId.trim()
          ? record.effectInstanceId
          : legacyEffectInstanceId(record.effectType, occurrence),
      effectType: record.effectType,
      enabled: record.enabled,
      params: record.params as Record<string, unknown>,
    }];
  });
}

export function nextCatalogEffectInstanceId(
  effectType: string,
  existingEffects: readonly ProjectActionEffect[],
) {
  const usedIds = new Set(existingEffects.map((effect) => effect.effectInstanceId));
  let occurrence = existingEffects.filter((effect) => effect.effectType === effectType).length;
  let candidate = legacyEffectInstanceId(effectType, occurrence);
  while (usedIds.has(candidate)) {
    occurrence += 1;
    candidate = legacyEffectInstanceId(effectType, occurrence);
  }
  return candidate;
}

export function itemNeedsCanonicalViewerPreparation(project: VideoProject, item: TimelineItem) {
  const mediaId = timelineItemSourceMediaId(item);
  if (mediaId && project.media.some((asset) => asset.id === mediaId && asset.kind === "lottie")) {
    return true;
  }
  const blendMode = item.properties.blendMode;
  if (typeof blendMode === "string" && !["normal", "over"].includes(blendMode)) {
    return true;
  }
  const effects = item.properties.effects;
  if (
    Array.isArray(effects) &&
    effects.some(
      (effect) =>
        effect !== null &&
        typeof effect === "object" &&
        !Array.isArray(effect) &&
        (effect as Record<string, unknown>).enabled !== false,
    )
  ) {
    return true;
  }
  const colorGrade = item.properties.colorGrade;
  return (
    colorGrade !== null &&
    typeof colorGrade === "object" &&
    !Array.isArray(colorGrade) &&
    Object.keys(colorGrade).length > 0
  );
}

export function clampTimelinePlayhead(seconds: number, durationSeconds: number) {
  if (!Number.isFinite(seconds) || !Number.isFinite(durationSeconds)) {
    return 0;
  }

  return Number(Math.min(Math.max(seconds, 0), Math.max(durationSeconds, 0)).toFixed(3));
}

export function clampPlayheadSecondsForTimeline(seconds: number, durationSeconds: number) {
  if (!Number.isFinite(seconds)) {
    return 0;
  }

  return Number(Math.min(Math.max(seconds, 0), durationSeconds).toFixed(3));
}

export function transformNumberProperty(item: TimelineItem, key: string): number | null {
  const transform = item.properties.transform;
  if (!transform || typeof transform !== "object" || Array.isArray(transform)) {
    return null;
  }
  const value = (transform as Record<string, unknown>)[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export function transformBooleanProperty(item: TimelineItem, key: string) {
  const transform = item.properties.transform;
  return Boolean(
    transform &&
      typeof transform === "object" &&
      !Array.isArray(transform) &&
      (transform as Record<string, unknown>)[key] === true,
  );
}

export function colorGradeNumberProperty(item: TimelineItem, key: string): number | null {
  const grade = item.properties.colorGrade;
  if (!grade || typeof grade !== "object" || Array.isArray(grade)) return null;
  const value = (grade as Record<string, unknown>)[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export function hasEffect(item: TimelineItem, effectType: string) {
  const effects = item.properties.effects;
  return Array.isArray(effects) && effects.some((effect) =>
    effect && typeof effect === "object" && (effect as Record<string, unknown>).effectType === effectType,
  );
}

export function audioDenoiseAmount(item: TimelineItem | null) {
  if (!item) return 0.6;
  const effects = item.properties.effects;
  if (!Array.isArray(effects)) return 0.6;
  const denoise = effects.find(
    (effect) =>
      effect &&
      typeof effect === "object" &&
      (effect as Record<string, unknown>).effectType === "audio.denoise" &&
      (effect as Record<string, unknown>).enabled !== false,
  ) as Record<string, unknown> | undefined;
  const params = denoise?.params;
  const amount =
    params && typeof params === "object" && !Array.isArray(params)
      ? (params as Record<string, unknown>).amount
      : null;
  return typeof amount === "number" && Number.isFinite(amount) ? amount : 0.6;
}

export function audioDenoisePreparationStatus(item: TimelineItem | null) {
  const preparation = item?.properties.audioDenoisePreparation;
  if (!preparation || typeof preparation !== "object" || Array.isArray(preparation)) {
    return item && hasEffect(item, "audio.denoise") ? "queued" : null;
  }
  const status = (preparation as Record<string, unknown>).status;
  return typeof status === "string" ? status : null;
}

function stableEffectInstanceId(effectType: string, index: number) {
  return legacyEffectInstanceId(effectType, index);
}

export type SourceEffectDrafts = Record<string, {
  effectInstanceId: string;
  enabled: boolean;
  params: Record<string, string>;
  resource: string;
}>;

export function sourceEffectDrafts(
  item: TimelineItem | null,
  visualEffectCatalog: readonly VisualEffectDescriptor[],
): SourceEffectDrafts {
  const existing = item ? projectActionEffectsForItem(item) : [];
  const drafts: SourceEffectDrafts = {};
  for (const descriptor of visualEffectCatalog) {
    const effect = existing.find((candidate) => candidate.effectType === descriptor.id);
    const params = Object.fromEntries(descriptor.params.map((param) => [
      param.key,
      String(typeof effect?.params[param.key] === "number" ? effect.params[param.key] : param.defaultValue),
    ]));
    if (descriptor.id === "color.curves") {
      const master = effect?.params.masterCurve;
      params.curveMidpoint = String(Array.isArray(master) && Array.isArray(master[1]) && typeof master[1][1] === "number" ? master[1][1] : 0.5);
    }
    if (descriptor.id === "color.hueCurves") {
      const target = Array.isArray(effect?.params.targets) && effect.params.targets[0] && typeof effect.params.targets[0] === "object"
        ? effect.params.targets[0] as Record<string, unknown> : {};
      for (const [key, fallback] of [["targetHue", 0], ["hueShift", 0], ["satScale", 1], ["lumShift", 0]] as const) {
        params[key] = String(typeof target[key] === "number" ? target[key] : fallback);
      }
    }
    drafts[descriptor.id] = {
      effectInstanceId: effect && "effectInstanceId" in effect && typeof effect.effectInstanceId === "string"
        ? effect.effectInstanceId
        : stableEffectInstanceId(descriptor.id, 0),
      enabled: effect?.enabled ?? false,
      params,
      resource: descriptor.resourceKey && typeof effect?.params[descriptor.resourceKey] === "string"
        ? String(effect.params[descriptor.resourceKey]) : "",
    };
  }
  return drafts;
}

export function isVisualOpacityItem(item: TimelineItem | null) {
  return (
    item?.kind === "video_clip" ||
    item?.kind === "image_clip" ||
    item?.kind === "lottie_clip" ||
    item?.kind === "generated_clip" ||
    item?.kind === "overlay" ||
    item?.kind === "hyperframe_scene"
  );
}

export function visualBlendModeProperty(item: TimelineItem): VisualBlendMode {
  const value = stringProperty(item, "blendMode");
  return visualBlendModes.find((mode) => mode === value) ?? "over";
}

export function isVisualOpacityClip(
  selectedTimelineClipContext: { kind: string } | null | undefined,
) {
  return (
    selectedTimelineClipContext?.kind === "video_clip" ||
    selectedTimelineClipContext?.kind === "overlay" ||
    selectedTimelineClipContext?.kind === "hyperframe_scene"
  );
}

export function isGeneratedTimelineItem(item: TimelineItem) {
  return (
    item.source.type === "generated" ||
    stringProperty(item, "generatedAssetId") !== null ||
    stringProperty(item, "generatedOutputMediaId") !== null
  );
}

export function generatedWorkflowStatusTitle(
  status: GeneratedTimelineWorkflowStatus,
) {
  return `Workflow ${status.status}, updated ${status.updatedAt}`;
}

function formatDecibels(value: number) {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}dB`;
}

export function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

export function visualClipOpacity(item: TimelineItem) {
  if (
    item.kind !== "video_clip" &&
    item.kind !== "overlay" &&
    item.kind !== "hyperframe_scene"
  ) {
    return null;
  }

  const opacity = numberProperty(item, "opacity");
  if (opacity === null || opacity < 0 || opacity >= 1) {
    return null;
  }

  return opacity;
}

export function itemMetadata(item: TimelineItem) {
  const sourceIn = numberProperty(item, "sourceIn");
  const sourceOut = numberProperty(item, "sourceOut");
  const reason = stringProperty(item, "reason");
  const fadeIn = audioFadeInSeconds(item);
  const fadeOut = audioFadeOutSeconds(item);
  const volumeDb =
    item.kind === "audio_clip" ? numberProperty(item, "volumeDb") : null;
  const opacity = visualClipOpacity(item);
  const reasonParts = [
    reason,
    fadeIn !== null ? `fade in ${formatSeconds(fadeIn)}` : null,
    fadeOut !== null ? `fade out ${formatSeconds(fadeOut)}` : null,
    volumeDb !== null ? `volume ${formatDecibels(volumeDb)}` : null,
    opacity !== null ? `opacity ${formatPercent(opacity)}` : null,
  ].filter((part): part is string => Boolean(part));
  const reasonText = reasonParts.length > 0 ? reasonParts.join(", ") : null;

  if (sourceIn !== null && sourceOut !== null) {
    return {
      range: `${formatSeconds(sourceIn)}-${formatSeconds(sourceOut)}`,
      reason: reasonText,
    };
  }

  if (item.source.type === "generated") {
    return {
      range: "generated",
      reason:
        reasonText ??
        stringProperty(item, "templateId") ??
        item.source.artifactId,
    };
  }

  return reasonText
    ? { range: item.kind.replace(/_/g, " "), reason: reasonText }
    : null;
}

export function audioFadeOutSeconds(item: TimelineItem) {
  if (item.kind !== "audio_clip") {
    return null;
  }

  const value = numberProperty(item, "fadeOutSeconds");
  if (value === null || value <= 0 || item.durationSeconds <= 0) {
    return null;
  }

  return Math.min(value, item.durationSeconds);
}

export function audioFadeInSeconds(item: TimelineItem) {
  if (item.kind !== "audio_clip") {
    return null;
  }

  const value = numberProperty(item, "fadeInSeconds");
  if (value === null || value <= 0 || item.durationSeconds <= 0) {
    return null;
  }

  return Math.min(value, item.durationSeconds);
}
