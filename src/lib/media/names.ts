import { formatSecondsShort } from "@/lib/format";
import type { MediaAsset } from "@/lib/project";

export function filenameFromPath(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

export function mediaDisplayName(media: MediaAsset) {
  return media.name?.trim() || filenameFromPath(media.relativePath);
}

export function greatestCommonDivisor(left: number, right: number): number {
  let a = Math.abs(Math.round(left));
  let b = Math.abs(Math.round(right));
  while (b > 0) {
    const next = a % b;
    a = b;
    b = next;
  }

  return a || 1;
}

/** Truncates instead of rounding and returns 0 (not 1) when both sides are 0. */
export function truncatedGreatestCommonDivisor(left: number, right: number): number {
  let a = Math.abs(Math.trunc(left));
  let b = Math.abs(Math.trunc(right));

  while (b > 0) {
    const next = a % b;
    a = b;
    b = next;
  }

  return a;
}

export function aspectRatioLabel(width: number, height: number) {
  const divisor = greatestCommonDivisor(width, height);
  return `${Math.round(width / divisor)}:${Math.round(height / divisor)}`;
}

export function formatAspectRatioOrNull(width: number | null | undefined, height: number | null | undefined) {
  if (!width || !height) {
    return null;
  }

  const divisor = greatestCommonDivisor(width, height);
  return `${Math.round(width / divisor)}:${Math.round(height / divisor)}`;
}

export function formatAspectRatioOrUnknown(width: number, height: number) {
  if (
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width <= 0 ||
    height <= 0
  ) {
    return "unknown";
  }

  const divisor = truncatedGreatestCommonDivisor(width, height);
  return `${Math.trunc(width / divisor)}:${Math.trunc(height / divisor)}`;
}

export function mediaQualityLabel(width: number | null | undefined, height: number | null | undefined) {
  if (!width || !height) {
    return null;
  }

  const longEdge = Math.max(width, height);
  const shortEdge = Math.min(width, height);

  if (longEdge >= 3840 || shortEdge >= 2160) {
    return "4K";
  }
  if (longEdge >= 2560 || shortEdge >= 1440) {
    return "QHD";
  }
  if (longEdge >= 1920 || shortEdge >= 1080) {
    return "FHD";
  }
  if (longEdge >= 1280 || shortEdge >= 720) {
    return "HD";
  }

  return "SD";
}

export function mediaAssetMeta(asset: MediaAsset) {
  const dimensions = asset.width && asset.height ? `${asset.width}x${asset.height}` : null;
  const fps = asset.fps ? `${Math.round(asset.fps)} fps` : null;

  return [asset.kind, dimensions, fps].filter(Boolean).join(" - ");
}

export function formatOptionalSeconds(seconds: number | null | undefined) {
  return typeof seconds === "number" && Number.isFinite(seconds)
    ? formatSecondsShort(seconds)
    : "unknown";
}

export function formatDimensions(width: number | null | undefined, height: number | null | undefined) {
  return width && height ? `${width} x ${height}` : "unknown";
}

export function formatFrameRate(fps: number | null | undefined) {
  return fps ? `${fps} fps` : "still";
}
