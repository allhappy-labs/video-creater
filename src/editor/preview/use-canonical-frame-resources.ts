import { useEffect, useMemo } from "react";
import type { PreparedProjectPreview } from "@/lib/project";
import { ensureRemoteMediaPaths } from "@/lib/runtime/adapters/remote-resource-cache";

/** Authorize a bounded decode window, including the next clip before its first frame. */
export function useCanonicalFrameResources(result: PreparedProjectPreview | null, projectDir: string, seconds: number): void {
  const paths = useMemo(() => canonicalFrameResourcePaths(result, seconds), [result, seconds]);
  useEffect(() => { void ensureRemoteMediaPaths(projectDir, paths); }, [projectDir, paths]);
}

export function canonicalFrameResourcePaths(result: PreparedProjectPreview | null, seconds: number): string[] {
  return (result?.frameSequences ?? []).flatMap((sequence) => {
    const local = seconds - sequence.startSeconds;
    if (!Number.isFinite(seconds) || sequence.fps <= 0 || local < -0.5 || local >= sequence.durationSeconds) return [];
    const index = Math.floor(Math.max(0, local * sequence.fps) / 12) * 12;
    return sequence.framePaths.slice(Math.max(0, index - 2), index + 24);
  });
}
