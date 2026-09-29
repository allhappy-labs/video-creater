export interface TimelineSnapProbe {
  seconds: number;
  edge: "start" | "end";
}

export interface TimelineSnapTarget {
  seconds: number;
  kind: "playhead" | "edit" | "seam";
}

export interface StickyTimelineSnap {
  targetSeconds: number;
  probeEdge: TimelineSnapProbe["edge"];
}

export interface TimelineSnapResult {
  deltaSeconds: number;
  guideSeconds: number | null;
  sticky: StickyTimelineSnap | null;
  targetKind: TimelineSnapTarget["kind"] | null;
}

interface ResolveTimelineSnapOptions {
  probes: readonly TimelineSnapProbe[];
  targets: readonly TimelineSnapTarget[];
  pixelsPerSecond: number;
  thresholdPixels?: number;
  releasePixels?: number;
  sticky?: StickyTimelineSnap | null;
}

const defaultThresholdPixels = 8;
const defaultReleasePixels = 14;

function finiteSeconds(value: number) {
  return Number.isFinite(value) ? Number(value.toFixed(3)) : null;
}

function targetPriority(kind: TimelineSnapTarget["kind"]) {
  if (kind === "playhead") return 0;
  if (kind === "seam") return 1;
  return 2;
}

export function resolveTimelineSnap({
  probes,
  targets,
  pixelsPerSecond,
  thresholdPixels = defaultThresholdPixels,
  releasePixels = defaultReleasePixels,
  sticky = null,
}: ResolveTimelineSnapOptions): TimelineSnapResult {
  if (!Number.isFinite(pixelsPerSecond) || pixelsPerSecond <= 0) {
    return { deltaSeconds: 0, guideSeconds: null, sticky: null, targetKind: null };
  }

  const validProbes = probes.flatMap((probe) => {
    const seconds = finiteSeconds(probe.seconds);
    return seconds === null ? [] : [{ ...probe, seconds }];
  });
  const validTargets = targets.flatMap((target) => {
    const seconds = finiteSeconds(target.seconds);
    return seconds === null ? [] : [{ ...target, seconds }];
  });

  if (sticky) {
    const probe = validProbes.find((candidate) => candidate.edge === sticky.probeEdge);
    const target = validTargets.find(
      (candidate) => Math.abs(candidate.seconds - sticky.targetSeconds) < 0.001,
    );
    if (probe && target) {
      const distancePixels = Math.abs(target.seconds - probe.seconds) * pixelsPerSecond;
      if (distancePixels <= releasePixels) {
        return {
          deltaSeconds: Number((target.seconds - probe.seconds).toFixed(3)),
          guideSeconds: target.seconds,
          sticky,
          targetKind: target.kind,
        };
      }
    }
  }

  const candidates = validProbes.flatMap((probe) =>
    validTargets.map((target) => ({
      probe,
      target,
      deltaSeconds: Number((target.seconds - probe.seconds).toFixed(3)),
      distancePixels: Math.abs(target.seconds - probe.seconds) * pixelsPerSecond,
    })),
  ).filter((candidate) => candidate.distancePixels <= thresholdPixels);

  candidates.sort((first, second) =>
    first.distancePixels - second.distancePixels ||
    targetPriority(first.target.kind) - targetPriority(second.target.kind) ||
    first.target.seconds - second.target.seconds ||
    (first.probe.edge === "start" ? -1 : 1),
  );

  const winner = candidates[0];
  if (!winner) {
    return { deltaSeconds: 0, guideSeconds: null, sticky: null, targetKind: null };
  }

  const nextSticky = {
    targetSeconds: winner.target.seconds,
    probeEdge: winner.probe.edge,
  } satisfies StickyTimelineSnap;
  return {
    deltaSeconds: winner.deltaSeconds,
    guideSeconds: winner.target.seconds,
    sticky: nextSticky,
    targetKind: winner.target.kind,
  };
}

export function timelineSnapTargets(
  editPoints: readonly number[],
  playheadSeconds: number,
): TimelineSnapTarget[] {
  const targets: TimelineSnapTarget[] = [
    { seconds: playheadSeconds, kind: "playhead" },
  ];
  const seen = new Set([Number(playheadSeconds.toFixed(3))]);
  for (const editPoint of editPoints) {
    const seconds = finiteSeconds(editPoint);
    if (seconds === null || seen.has(seconds)) continue;
    seen.add(seconds);
    targets.push({ seconds, kind: "edit" });
  }
  return targets;
}
