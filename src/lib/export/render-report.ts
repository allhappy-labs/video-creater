import type { ProjectRenderReport } from "@/lib/project";

/**
 * Codec exports (MP4, MOV) record their delivery quality and container in the summary; the
 * WebM-only quality profiles do not describe them.
 */
export function deliveryQualityLabel(summary: { quality?: "draft" | "final" | null; container?: string | null }) {
  const container = summary.container?.trim().toLowerCase();
  if (!summary.quality || !container || container === "webm") return null;
  return `${summary.quality === "draft" ? "Draft" : "Final"} ${container.toUpperCase()}`;
}

export function qaMetricEntries(metrics: Record<string, string>) {
  return Object.entries(metrics).sort(([left], [right]) => left.localeCompare(right));
}

export function normalizeArtifactPath(path: string) {
  return path.replace(/\\/g, "/");
}

export function artifactBackedFrameCount(sampledFrames: string[], artifacts: string[]) {
  const normalizedArtifacts = artifacts.map(normalizeArtifactPath);
  return sampledFrames.filter((frame) => {
    const normalizedFrame = normalizeArtifactPath(frame);
    return normalizedArtifacts.some(
      (artifact) => artifact === normalizedFrame || artifact.endsWith(`/${normalizedFrame}`),
    );
  }).length;
}

export function formatTimelineSeconds(seconds: number) {
  return `${seconds.toFixed(2).replace(/\.?0+$/, "")}s`;
}

export function formatMismatchRatio(ratio: number) {
  return `${(ratio * 100).toFixed(2)}% diff`;
}

export function renderCheckLabel(check: string) {
  return check
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .trim()
    .toLowerCase();
}

export function renderCheckEntries(report: ProjectRenderReport) {
  return Object.entries(report.checks).sort(([left], [right]) => left.localeCompare(right));
}
