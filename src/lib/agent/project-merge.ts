import { mergeProjectJobs } from "@/lib/jobs/activity-records";
import { applyProjectActionLocally, type ProjectAction, type VideoProject } from "@/lib/project";

export function applyProjectActionsLocally(
  baseProject: VideoProject,
  actions: ProjectAction[],
): VideoProject {
  return actions.reduce(
    (currentProject, action) => applyProjectActionLocally(currentProject, action),
    baseProject,
  );
}

export function mergeCodexProjectMetadata(
  currentProject: VideoProject,
  codexProject: VideoProject,
): VideoProject {
  if (currentProject.id !== codexProject.id) {
    return codexProject;
  }

  const mediaAnalysis = codexProject.mediaAnalysis ?? currentProject.mediaAnalysis;
  const contentRevision = codexProject.contentRevision ?? currentProject.contentRevision;
  return {
    ...currentProject,
    ...(contentRevision === undefined ? {} : { contentRevision }),
    codexThreadId: codexProject.codexThreadId ?? currentProject.codexThreadId,
    ...(mediaAnalysis === undefined ? {} : { mediaAnalysis }),
    jobs: mergeProjectJobs(currentProject.jobs, codexProject.jobs),
  };
}

export function mergeProjectMediaAnalysis(
  currentProject: VideoProject,
  analyzedProject: VideoProject,
): VideoProject {
  if (currentProject.id !== analyzedProject.id) {
    return analyzedProject;
  }

  const mediaAnalysis = analyzedProject.mediaAnalysis ?? currentProject.mediaAnalysis;
  const contentRevision =
    currentProject.contentRevision === undefined
      ? analyzedProject.contentRevision
      : analyzedProject.contentRevision === undefined
        ? currentProject.contentRevision
        : Math.max(currentProject.contentRevision, analyzedProject.contentRevision);
  return {
    ...currentProject,
    ...(contentRevision === undefined ? {} : { contentRevision }),
    ...(mediaAnalysis === undefined ? {} : { mediaAnalysis }),
  };
}

export function projectHistorySnapshot(project: VideoProject) {
  return structuredClone(project);
}

export function projectSnapshotsEqual(left: VideoProject, right: VideoProject) {
  return JSON.stringify(left) === JSON.stringify(right);
}
