import { formatDurationLabel as formatPreviewDuration, formatSecondsShort } from "@/lib/format";
import { aspectRatioLabel, mediaDisplayName, mediaQualityLabel } from "@/lib/media/names";
import type { GeneratedAsset, MediaAsset, VideoProject } from "@/lib/project";
import { backendMediaUrl } from "@/lib/runtime/backend-client";
import { sampleProjectBrowserPreviewUrl } from "@/lib/sample-project";
import type { Timeline, TimelineItem } from "@/lib/timeline";
import { numberProperty } from "@/lib/timeline-ops/item-properties";

export type ViewerMode = "timeline" | "source";

export interface PreviewSource {
  id: string;
  label: string;
  kind: string;
  durationLabel: string | null;
  resolutionLabel: string | null;
  fpsLabel: string | null;
  aspectRatioLabel?: string | null;
  qualityLabel?: string | null;
  pathLabel: string;
  sourceRangeLabel?: string | null;
  previewUrl?: string | null;
}

export function sourceViewerTabId(sourceId: string) {
  return `preview-viewer-tab-source-${encodeURIComponent(sourceId)}`;
}

export function parsePreviewDurationLabel(durationLabel: string | null | undefined) {
  if (!durationLabel) {
    return 0;
  }

  const parts = durationLabel.split(":").map((part) => Number.parseFloat(part));
  if (parts.some((part) => !Number.isFinite(part) || part < 0)) {
    return 0;
  }

  if (parts.length === 2) {
    const [minutes, seconds] = parts;
    if (minutes === undefined || seconds === undefined) return 0;
    return minutes * 60 + seconds;
  }

  if (parts.length === 3) {
    const [hours, minutes, seconds] = parts;
    if (hours === undefined || minutes === undefined || seconds === undefined) return 0;
    return hours * 3600 + minutes * 60 + seconds;
  }

  return 0;
}

export function formatPreviewCurrentTime(totalSeconds: number) {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) {
    return "00:00:00";
  }

  const totalMilliseconds = Math.round(totalSeconds * 1000);
  const hours = Math.floor(totalMilliseconds / 3_600_000);
  const minutes = Math.floor((totalMilliseconds % 3_600_000) / 60_000);
  const seconds = Math.floor((totalMilliseconds % 60_000) / 1000);
  const milliseconds = totalMilliseconds % 1000;
  const wholeSecondsLabel = `${hours.toString().padStart(2, "0")}:${minutes
    .toString()
    .padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;

  if (milliseconds === 0) {
    return wholeSecondsLabel;
  }

  return `${wholeSecondsLabel}.${milliseconds.toString().padStart(3, "0")}`;
}

export function previewTransportData(input: {
  activeViewerMode: ViewerMode;
  timeline?: Timeline;
  selectedItem: TimelineItem | null | undefined;
  selectedSource: PreviewSource | null;
}) {
  if (input.activeViewerMode === "source" && input.selectedSource) {
    return {
      durationLabel: input.selectedSource.durationLabel ?? "00:00",
      primaryLabel: input.selectedSource.sourceRangeLabel ?? null,
      badges: [
        input.selectedSource.resolutionLabel,
        input.selectedSource.aspectRatioLabel,
        input.selectedSource.fpsLabel,
        input.selectedSource.qualityLabel,
        "Fit",
      ].filter(Boolean),
    };
  }

  if (input.timeline) {
    return {
      durationLabel: formatPreviewDuration(input.timeline.durationSeconds),
      primaryLabel: null,
      badges: ["Fit"],
    };
  }

  if (input.selectedItem) {
    return {
      durationLabel: formatPreviewDuration(input.selectedItem.durationSeconds),
      primaryLabel: null,
      badges: ["Fit"],
    };
  }

  return {
    durationLabel: "00:00",
    primaryLabel: null,
    badges: ["Fit"],
  };
}

export function sourcePreviewStepSeconds(source: PreviewSource | null) {
  if (!source) {
    return 0;
  }

  if (source.kind === "audio") {
    return 1;
  }

  if (source.kind === "video" || source.kind === "generated") {
    const fps = Number.parseFloat(source.fpsLabel ?? "");
    return Number.isFinite(fps) && fps > 0 ? 1 / fps : 1 / 24;
  }

  return 0;
}

export function mediaElementDurationSeconds(
  mediaElement: HTMLMediaElement | null,
  source: PreviewSource | null,
) {
  if (
    mediaElement &&
    Number.isFinite(mediaElement.duration) &&
    mediaElement.duration > 0
  ) {
    return mediaElement.duration;
  }

  return parsePreviewDurationLabel(source?.durationLabel);
}

export function safeProjectMediaPath(projectDir: string, relativePath: string) {
  const trimmedProjectDir = projectDir.trim().replace(/\\/g, "/").replace(/\/+$/, "");
  const trimmedRelativePath = relativePath.trim();
  if (!trimmedProjectDir || !trimmedRelativePath) {
    return null;
  }
  if (
    trimmedRelativePath.startsWith("/") ||
    trimmedRelativePath.startsWith("\\") ||
    /^[A-Za-z][A-Za-z0-9+.-]*:/.test(trimmedRelativePath)
  ) {
    return null;
  }

  const parts: string[] = [];
  for (const part of trimmedRelativePath.replace(/\\/g, "/").split("/")) {
    if (!part || part === ".") {
      continue;
    }
    if (part === "..") {
      return null;
    }
    parts.push(part);
  }

  return parts.length > 0 ? `${trimmedProjectDir}/${parts.join("/")}` : null;
}

export function previewUrlForMedia(projectDir: string, relativePath: string) {
  const bundledBrowserUrl = sampleProjectBrowserPreviewUrl(projectDir, relativePath);
  if (bundledBrowserUrl) return bundledBrowserUrl;
  const safePath = safeProjectMediaPath(projectDir, relativePath);
  if (!safePath) {
    return null;
  }

  try {
    return backendMediaUrl(safePath);
  } catch {
    return null;
  }
}

export function formatViewerDuration(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return null;
  }

  const roundedSeconds = Math.round(seconds);
  const minutes = Math.floor(roundedSeconds / 60);
  const remainingSeconds = roundedSeconds % 60;

  return `${minutes.toString().padStart(2, "0")}:${remainingSeconds
    .toString()
    .padStart(2, "0")}`;
}

export function sourceRangeLabelForItem(item: TimelineItem) {
  const sourceIn = numberProperty(item, "sourceIn");
  const sourceOut = numberProperty(item, "sourceOut");
  if (sourceIn === null || sourceOut === null) {
    return null;
  }

  return `Source ${formatSecondsShort(sourceIn)}-${formatSecondsShort(sourceOut)}`;
}

export function previewUrlsForMedia(projectDir: string, media: readonly MediaAsset[]) {
  return Object.fromEntries(
    media.map((asset) => [asset.id, previewUrlForMedia(projectDir, asset.relativePath)]),
  );
}

export function previewUrlsForTimelineSources(
  projectDir: string,
  media: readonly MediaAsset[],
  generatedAssets: readonly GeneratedAsset[],
) {
  const urls = previewUrlsForMedia(projectDir, media);
  for (const generatedAsset of generatedAssets) {
    for (const output of generatedAsset.outputs) {
      if (!(output.mediaId in urls)) {
        urls[output.mediaId] = previewUrlForMedia(projectDir, output.relativePath);
      }
    }
  }

  return urls;
}

export function selectedPreviewSource(
  project: VideoProject,
  mediaId: string | null,
  sourceRangeLabel: string | null = null,
  projectDir = "",
): PreviewSource | null {
  if (!mediaId) {
    return null;
  }

  const media = project.media.find((asset) => asset.id === mediaId) ?? null;
  if (!media) {
    return null;
  }

  const resolutionLabel = media.width && media.height ? `${media.width}x${media.height}` : null;
  const previewAspectRatioLabel =
    media.width && media.height ? aspectRatioLabel(media.width, media.height) : null;
  const fpsLabel = media.fps ? `${Math.round(media.fps)} fps` : null;
  return {
    id: media.id,
    label: mediaDisplayName(media),
    kind: media.kind,
    durationLabel: formatViewerDuration(media.durationSeconds),
    resolutionLabel,
    aspectRatioLabel: previewAspectRatioLabel,
    fpsLabel,
    qualityLabel: mediaQualityLabel(media.width, media.height),
    pathLabel: media.relativePath,
    sourceRangeLabel,
    previewUrl: previewUrlForMedia(projectDir, media.relativePath),
  };
}

export function sourceRangeLabel(item: TimelineItem) {
  const sourceIn = numberProperty(item, "sourceIn");
  const sourceOut = numberProperty(item, "sourceOut");
  if (sourceIn === null || sourceOut === null) {
    return null;
  }

  return `${formatSecondsShort(sourceIn)}-${formatSecondsShort(sourceOut)}`;
}
