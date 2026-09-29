import { filenameFromPath } from "@/lib/media/names";
import type {
  GeneratedAsset,
  GeneratedAssetSettings,
  MediaAsset,
  ProjectJobSummary,
  VideoProject,
} from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import {
  stringProperty,
  timelineItemSourceMediaId,
  timelineItemSourceMediaId as sourceMediaId,
} from "@/lib/timeline-ops/item-properties";

export function uniqueStringValues(values: Array<string | null | undefined>) {
  return Array.from(new Set(values.filter((value): value is string => Boolean(value))));
}

export function generatedReferenceMediaIds(references: GeneratedAsset["references"]) {
  return uniqueStringValues([
    ...references.mediaIds,
    ...(references.referenceImageMediaRefs ?? []),
    ...(references.referenceVideoMediaRefs ?? []),
    ...(references.referenceAudioMediaRefs ?? []),
  ]);
}

export function generatedAssetForTimelineItem(project: VideoProject, item: TimelineItem) {
  const generatedAssetId = stringProperty(item, "generatedAssetId");
  if (generatedAssetId) {
    const explicitAsset =
      project.generatedAssets.find((asset) => asset.id === generatedAssetId) ?? null;
    if (explicitAsset) {
      return explicitAsset;
    }
  }

  const sourceMediaId = timelineItemSourceMediaId(item);
  if (!sourceMediaId) {
    return null;
  }

  return (
    project.generatedAssets.find(
      (asset) =>
        asset.id === sourceMediaId ||
        asset.outputs.some((output) => output.mediaId === sourceMediaId),
    ) ?? null
  );
}

export function generatedAssetTitleWithPrompt(asset: GeneratedAsset) {
  const name = asset.name?.trim();
  if (name) {
    return name;
  }

  const firstPromptLine = asset.prompt
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);

  return firstPromptLine ?? asset.id;
}

export function findGeneratedAssetForItem(
  item: TimelineItem,
  generatedAssets: readonly GeneratedAsset[],
) {
  const generatedAssetId = stringProperty(item, "generatedAssetId");
  if (generatedAssetId) {
    const explicitAsset = generatedAssets.find((asset) => asset.id === generatedAssetId);
    if (explicitAsset) {
      return explicitAsset;
    }
  }

  const mediaId = sourceMediaId(item);
  if (!mediaId) {
    return null;
  }

  return generatedAssets.find(
    (asset) => generatedAssetHasMedia(asset, mediaId),
  ) ?? null;
}

export function generatedAssetHasMedia(asset: GeneratedAsset, mediaId: string) {
  return asset.id === mediaId || asset.outputs.some((output) => output.mediaId === mediaId);
}

export function findGeneratedAssetForMedia(
  mediaId: string | null,
  generatedAssets: readonly GeneratedAsset[],
) {
  if (!mediaId) {
    return null;
  }

  return (
    generatedAssets.find((asset) => generatedAssetHasMedia(asset, mediaId)) ?? null
  );
}

export function findGeneratedOutputForItem(item: TimelineItem, generatedAsset: GeneratedAsset | null) {
  if (!generatedAsset) {
    return null;
  }

  const outputMediaId = stringProperty(item, "generatedOutputMediaId") ?? sourceMediaId(item);
  return (
    generatedAsset.outputs.find((output) => output.mediaId === outputMediaId) ??
    generatedAsset.outputs[0] ??
    null
  );
}

export function findGeneratedOutputForMedia(
  mediaId: string | null,
  generatedAsset: GeneratedAsset | null,
) {
  if (!generatedAsset) {
    return null;
  }

  return (
    generatedAsset.outputs.find((output) => output.mediaId === mediaId) ??
    generatedAsset.outputs[0] ??
    null
  );
}

export function findMediaAsset(media: readonly MediaAsset[], mediaId: string | null | undefined) {
  return mediaId ? media.find((asset) => asset.id === mediaId) ?? null : null;
}

export function generatedAssetCreatedAtMs(asset: GeneratedAsset) {
  const timestamp = Date.parse(asset.createdAt);
  return Number.isFinite(timestamp) ? timestamp : null;
}

export function sortGeneratedAssetsByCreatedAtDesc(assets: readonly GeneratedAsset[]) {
  return assets
    .map((asset, index) => ({
      asset,
      index,
      createdAtMs: generatedAssetCreatedAtMs(asset),
    }))
    .sort((left, right) => {
      if (left.createdAtMs === null && right.createdAtMs === null) {
        return left.index - right.index;
      }

      if (left.createdAtMs === null) {
        return 1;
      }

      if (right.createdAtMs === null) {
        return -1;
      }

      return right.createdAtMs - left.createdAtMs || left.index - right.index;
    })
    .map(({ asset }) => asset);
}

export function generatedAssetTitleOrPrompt(asset: GeneratedAsset) {
  return asset.name?.trim() || asset.prompt;
}

export function generatedPendingOutputLabel(asset: GeneratedAsset) {
  if (asset.outputs.length > 0) {
    return null;
  }

  return asset.status === "queued" || asset.status === "running"
    ? "waiting for generated output"
    : "no outputs yet";
}

export function isActiveGeneratedAsset(asset: GeneratedAsset) {
  return asset.status === "queued" || asset.status === "running";
}

export function generatedOutputHasProviderSourceUrl(output: GeneratedAsset["outputs"][number]) {
  return Boolean(output.sourceUrl?.trim());
}

export function generatedAssetNeedsHistoryCard(asset: GeneratedAsset, mediaIds: Set<string>) {
  if (asset.status !== "completed") {
    return true;
  }

  if (asset.outputs.length === 0) {
    return true;
  }

  return asset.outputs.some(
    (output) =>
      !mediaIds.has(output.mediaId) || generatedOutputHasProviderSourceUrl(output),
  );
}

export function humanizeWorkflowToken(value: string) {
  return value.replace(/[_-]+/g, " ").trim();
}

export function generatedWorkflowLabel(job: ProjectJobSummary) {
  return `Workflow ${humanizeWorkflowToken(job.status)} - ${humanizeWorkflowToken(job.kind)}`;
}

export function findGeneratedAssetForOutput(
  assets: readonly GeneratedAsset[],
  mediaId: string | null,
) {
  if (!mediaId) {
    return null;
  }

  return assets.find((asset) => asset.outputs.some((output) => output.mediaId === mediaId)) ?? null;
}

export function formatGeneratedResolution(width: number | null, height: number | null) {
  return width && height ? `${width} x ${height}` : "Auto";
}

export function formatGeneratedDuration(seconds: number | null) {
  if (!seconds || seconds <= 0) {
    return "Auto";
  }

  return Number.isInteger(seconds) ? `${seconds}s` : `${seconds.toFixed(1)}s`;
}

export function generatedOutputFileLabel(output: GeneratedAsset["outputs"][number]) {
  return filenameFromPath(output.relativePath);
}

export function generatedOutputMeta(
  output: GeneratedAsset["outputs"][number],
  mediaAsset: MediaAsset | null,
) {
  const width = mediaAsset?.width ?? output.width;
  const height = mediaAsset?.height ?? output.height;
  const durationSeconds = mediaAsset?.durationSeconds ?? output.durationSeconds;
  const fps = mediaAsset?.fps ?? output.fps;

  return [
    formatGeneratedResolution(width, height),
    formatGeneratedDuration(durationSeconds),
    fps ? `${Math.round(fps)} fps` : null,
  ]
    .filter(Boolean)
    .join(" - ");
}

export function generatedAssetTitleOrId(asset: VideoProject["generatedAssets"][number]) {
  return asset.name?.trim() || asset.id;
}

export function generatedReferenceCount(asset: VideoProject["generatedAssets"][number]) {
  return (
    asset.references.mediaIds.length +
    (asset.references.firstFrameMediaId ? 1 : 0) +
    (asset.references.lastFrameMediaId ? 1 : 0)
  );
}

export function generatedModelLabel(asset: VideoProject["generatedAssets"][number]) {
  return `${asset.model.provider}/${asset.model.id}`;
}

export function promptExcerpt(prompt: string) {
  const normalized = prompt.trim().replace(/\s+/g, " ");
  return normalized.length > 130 ? `${normalized.slice(0, 127)}...` : normalized;
}

export function recentGeneratedAssets(project: VideoProject) {
  return [...project.generatedAssets]
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
    .slice(0, 3);
}

export function selectedGeneratedClipSettingsLabels(settings: GeneratedAssetSettings | null | undefined) {
  if (!settings) {
    return [];
  }

  const labels: string[] = [];
  if (
    typeof settings.width === "number" &&
    Number.isFinite(settings.width) &&
    settings.width > 0 &&
    typeof settings.height === "number" &&
    Number.isFinite(settings.height) &&
    settings.height > 0
  ) {
    labels.push(`${Math.round(settings.width)}x${Math.round(settings.height)}`);
  }
  if (
    typeof settings.durationSeconds === "number" &&
    Number.isFinite(settings.durationSeconds) &&
    settings.durationSeconds > 0
  ) {
    labels.push(
      `${Number.isInteger(settings.durationSeconds) ? settings.durationSeconds : settings.durationSeconds.toFixed(1)}s`,
    );
  }
  if (typeof settings.fps === "number" && Number.isFinite(settings.fps) && settings.fps > 0) {
    labels.push(`${Number.isInteger(settings.fps) ? settings.fps : settings.fps.toFixed(2)} fps`);
  }
  if (typeof settings.aspectRatio === "string" && settings.aspectRatio.trim()) {
    labels.push(settings.aspectRatio.trim());
  }

  return labels;
}
