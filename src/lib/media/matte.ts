export const matteAspectOptions = [
  "Project",
  "16:9",
  "9:16",
  "1:1",
  "4:3",
  "9:14",
  "2.4:1",
] as const;

export type MatteAspectRatio = (typeof matteAspectOptions)[number];

export interface MatteCreateInput {
  hex: string;
  aspectRatio: MatteAspectRatio;
}

export function evenSize(value: number) {
  const finiteValue = Number.isFinite(value) ? value : 2;
  return Math.max(2, Math.floor(Math.max(2, finiteValue) / 2) * 2);
}

export function mattePreviewSize(
  aspectRatio: MatteAspectRatio,
  timelineWidth: number,
  timelineHeight: number,
) {
  const width = Math.max(2, timelineWidth);
  const height = Math.max(2, timelineHeight);
  if (aspectRatio === "Project") {
    return [evenSize(width), evenSize(height)] as const;
  }
  const [aspectWidth, aspectHeight] =
    aspectRatio === "2.4:1"
      ? [24, 10]
      : aspectRatio.split(":").map(Number);
  if (!aspectWidth || !aspectHeight) {
    return [evenSize(width), evenSize(height)] as const;
  }
  const shortEdge = Math.max(2, Math.min(width, height));
  return aspectWidth >= aspectHeight
    ? [evenSize(Math.round((shortEdge * aspectWidth) / aspectHeight)), evenSize(shortEdge)] as const
    : [evenSize(shortEdge), evenSize(Math.round((shortEdge * aspectHeight) / aspectWidth))] as const;
}

export function normalizedHex(value: string) {
  const trimmed = value.trim();
  return /^#[0-9a-f]{6}$/i.test(trimmed) ? trimmed.toUpperCase() : null;
}
