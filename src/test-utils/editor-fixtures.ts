import type {
  GeneratedAsset,
  MediaAsset,
  ProjectJobSummary,
  VideoProject,
} from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { requiredValue } from "./required";

/** A fresh deep copy of the bundled sample project; safe to mutate per test. */
export function fixtureProject(): VideoProject {
  return structuredClone(createSampleProject());
}

export function fixtureMedia(project: VideoProject, kind: MediaAsset["kind"]): MediaAsset {
  return requiredValue(
    project.media.find((media) => media.kind === kind),
    `sample project media of kind ${kind}`,
  );
}

export function fixtureTrack(project: VideoProject, kind: TimelineTrack["kind"]): TimelineTrack {
  return requiredValue(
    project.timeline.tracks.find((track) => track.kind === kind),
    `sample project track of kind ${kind}`,
  );
}

export function fixtureItem(project: VideoProject, trackKind: TimelineTrack["kind"]): TimelineItem {
  return requiredValue(fixtureTrack(project, trackKind).items[0], `first item on ${trackKind} track`);
}

export function fixtureGeneratedAsset(project: VideoProject): GeneratedAsset {
  return requiredValue(project.generatedAssets[0], "sample project generated asset");
}

export function fixtureJobs(project: VideoProject): ProjectJobSummary[] {
  return project.jobs;
}
