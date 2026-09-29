import {
  orderRecentProjectJobs,
  type CodexEditProposal,
  type JobExportSettings,
  type ProjectExportArtifact,
  type ProjectJobStatus,
  type ProjectJobSummary,
  type ProjectRenderReport,
  type VideoProject,
} from "@/lib/project";
import { formatAspectRatioOrUnknown as formatAspectRatio } from "@/lib/media/names";

export interface ActivityJobRecord {
  job: ProjectJobSummary;
  targetLabel: string | null;
  proposalAvailable: boolean;
  proposal: CodexEditProposal | null;
  outputPath: string | null;
  reportId: string | null;
  logPath: string | null;
}

export function nonEmpty(value: string | null | undefined) {
  const normalized = value?.trim();
  return normalized ? normalized : null;
}

export function embeddedCodexProposal(job: ProjectJobSummary): CodexEditProposal | null {
  const value = job.startRequest?.input.proposal;
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const proposal = value as Partial<CodexEditProposal>;
  const renderReview = proposal.renderReview;
  if (
    typeof proposal.mediaId !== "string" ||
    !Array.isArray(proposal.clips) ||
    !proposal.clips.every(
      (clip) =>
        clip &&
        typeof clip === "object" &&
        typeof clip.sourceIn === "number" &&
        typeof clip.sourceOut === "number" &&
        typeof clip.reason === "string",
    ) ||
    !Array.isArray(proposal.captions) ||
    !Array.isArray(proposal.overlays) ||
    !Array.isArray(proposal.hyperframes) ||
    !Array.isArray(proposal.gpuVisuals) ||
    !Array.isArray(proposal.projectActions) ||
    !renderReview ||
    typeof renderReview.durationSeconds !== "number" ||
    typeof renderReview.streamCheckRequired !== "boolean" ||
    typeof renderReview.captionAlignmentRequired !== "boolean" ||
    typeof renderReview.overlayTimingRequired !== "boolean" ||
    typeof renderReview.visualFrameEvidenceRequired !== "boolean" ||
    typeof renderReview.artifactPathsRequired !== "boolean" ||
    typeof renderReview.logReferenceRequired !== "boolean"
  ) {
    return null;
  }
  return value as CodexEditProposal;
}

export function buildActivityJobRecords(project: VideoProject): ActivityJobRecord[] {
  const generatedAssetsById = new Map(
    project.generatedAssets.map((asset) => [asset.id, asset] as const),
  );
  const renderReportsById = new Map(
    project.renderReports.map((report) => [report.id, report] as const),
  );
  const exportArtifactsByJobId = new Map(
    (project.exportArtifacts ?? [])
      .filter((artifact) => nonEmpty(artifact.jobId))
      .map((artifact) => [artifact.jobId as string, artifact] as const),
  );
  const recordsByJobId = new Map(
    project.jobs.map((job) => {
      const generatedAsset = generatedAssetsById.get(generationJobAssetId(job)) ?? null;
      const renderReport = renderReportsById.get(job.id) ?? null;
      const exportArtifact = exportArtifactsByJobId.get(job.id) ?? null;
      const generatedOutputPath = generatedAsset?.outputs
        .map((output) => nonEmpty(output.relativePath))
        .find((path): path is string => Boolean(path)) ?? null;
      const proposal = embeddedCodexProposal(job);
      const record: ActivityJobRecord = {
        job,
        targetLabel: generatedAsset
          ? nonEmpty(generatedAsset.name) ?? generatedAsset.id
          : null,
        proposalAvailable: Boolean(proposal),
        proposal,
        // The saved export file, when there is one, is what the user asked for.
        outputPath:
          nonEmpty(exportArtifact?.path) ??
          nonEmpty(renderReport?.outputPath) ??
          generatedOutputPath,
        reportId: renderReport ? renderReport.id : null,
        logPath: nonEmpty(renderReport?.logPath),
      };
      return [job.id, record] as const;
    }),
  );

  return orderRecentProjectJobs(project.jobs, project.jobs.length)
    .map((job) => recordsByJobId.get(job.id))
    .filter((record): record is ActivityJobRecord => Boolean(record));
}

export function formatJobKindTitleCase(kind: string) {
  return kind
    .split("_")
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join(" ");
}

export function formatJobKindSentenceCase(kind: string) {
  const words = kind
    .split("_")
    .filter(Boolean)
    .map((segment) => segment.toLowerCase());

  return words
    .map((word, index) => (index === 0 ? word.charAt(0).toUpperCase() + word.slice(1) : word))
    .join(" ");
}

export function formatJobStatus(status: ProjectJobStatus) {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

export function activeJobCount(project: VideoProject) {
  return project.jobs.filter((job) =>
    ["blocked", "progress", "queued", "running"].includes(job.status),
  ).length;
}

export function activeGenerationJobCount(project: VideoProject) {
  return project.jobs.filter(
    (job) =>
      job.kind === "generate_media" &&
      ["blocked", "progress", "queued", "running"].includes(job.status),
  ).length;
}

export function latestRenderReport(project: VideoProject): ProjectRenderReport | null {
  return project.renderReports.at(-1) ?? null;
}

export function latestExportArtifact(project: VideoProject): ProjectExportArtifact | null {
  return project.exportArtifacts?.at(-1) ?? null;
}

export function recentExportArtifacts(project: VideoProject) {
  return [...(project.exportArtifacts ?? [])]
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
    .slice(0, 3);
}

export function startRequestStatus(job: ProjectJobSummary) {
  const startRequest = job.startRequest ?? null;
  if (!startRequest) {
    return null;
  }

  const workflow = job.workflow ?? null;
  if (!workflow) {
    return "recorded";
  }

  const activityTypesMatch =
    startRequest.activityTypes.length === workflow.activityTypes.length &&
    startRequest.activityTypes.every(
      (activityType, index) => activityType === workflow.activityTypes[index],
    );

  return startRequest.workflowId === workflow.workflowId &&
    startRequest.workflowType === workflow.workflowType &&
    startRequest.taskQueue === workflow.taskQueue &&
    activityTypesMatch
    ? "ready"
    : "mismatch";
}

export function canShowStartWorkflowAction(status: ProjectJobStatus) {
  return status === "queued" || status === "blocked";
}

export function startRequestStringInput(job: ProjectJobSummary, key: string) {
  const value = job.startRequest?.input[key];
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

const videoExportProfiles: readonly string[] = ["webm", "mp4H264", "mp4H265", "proResMov"];

/**
 * A job's recorded export settings, else the settings its Temporal start request carries (jobs
 * recorded before export settings existed); null when neither names a complete video export.
 */
export function jobExportSettings(job: ProjectJobSummary): JobExportSettings | null {
  if (job.exportSettings) return job.exportSettings;
  const input = job.startRequest?.input;
  const profile = startRequestStringInput(job, "profile");
  const quality = startRequestStringInput(job, "quality");
  const width = input?.width;
  const height = input?.height;
  if (!input || !profile || !videoExportProfiles.includes(profile) || (quality !== "draft" && quality !== "final")) return null;
  if (typeof width !== "number" || typeof height !== "number") return null;
  const destination = input.destination as { fileName?: unknown; directory?: unknown } | undefined;
  const output =
    destination && typeof destination.fileName === "string"
      ? { fileName: destination.fileName, ...(typeof destination.directory === "string" ? { directory: destination.directory } : {}) }
      : null;
  return {
    profile: profile as JobExportSettings["profile"],
    quality,
    width,
    height,
    ...(typeof input.fps === "number" ? { fps: input.fps } : {}),
    ...(input.encodeTier === "master" ? { encodeTier: "master" as const } : {}),
    ...(output ? { output } : {}),
  };
}

/**
 * The generated asset a job belongs to. The editor records generation jobs under their asset id, but
 * agent bundles record `job-<assetId>` jobs, so a generation's start request `assetId` wins.
 */
export function generationJobAssetId(job: ProjectJobSummary): string {
  return (job.kind === "generate_media" ? startRequestStringInput(job, "assetId") : null) ?? job.id;
}

export function startRequestValidationInput(job: ProjectJobSummary) {
  const value = job.startRequest?.input.validation;
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

export function validationString(validation: Record<string, unknown>, key: string) {
  const value = validation[key];
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

export function startRequestVideoValidationSummary(job: ProjectJobSummary) {
  const validation = startRequestValidationInput(job);
  if (!validation) {
    return null;
  }

  const container = validationString(validation, "container");
  const videoCodec = validationString(validation, "videoCodec");
  const mimeType = validationString(validation, "mimeType");

  return container && videoCodec && mimeType
    ? `${container} / ${videoCodec} / ${mimeType}`
    : null;
}

export function startRequestAudioValidationSummary(job: ProjectJobSummary) {
  const validation = startRequestValidationInput(job);
  if (!validation) {
    return null;
  }

  const audioCodec = validationString(validation, "audioCodec");
  return audioCodec ? `audio ${audioCodec}` : null;
}

export function projectResolutionLabel(project: VideoProject) {
  return `${project.renderSettings.width} x ${project.renderSettings.height}`;
}

export function projectFrameRateLabel(project: VideoProject) {
  return `${project.renderSettings.fps} fps`;
}

export function projectAspectRatioLabel(project: VideoProject) {
  return formatAspectRatio(
    project.renderSettings.width,
    project.renderSettings.height,
  );
}

const finishedJobStatuses: ReadonlySet<string> = new Set(["completed", "failed", "cancelled"]);

/**
 * A finished job from the same workflow run always supersedes that run's unfinished copy. Temporal
 * workflows stamp their job updates with the workflow start time, which can be earlier than the
 * editor's own record of the started run.
 */
function finishesSameWorkflowRun(incoming: ProjectJobSummary, current: ProjectJobSummary): boolean {
  const runId = incoming.workflow?.runId;
  return Boolean(runId) && runId === current.workflow?.runId && finishedJobStatuses.has(incoming.status) && !finishedJobStatuses.has(current.status);
}

export function mergeProjectJobs(
  currentJobs: ProjectJobSummary[],
  incomingJobs: ProjectJobSummary[],
): ProjectJobSummary[] {
  const jobsById = new Map(currentJobs.map((job) => [job.id, job]));
  for (const job of incomingJobs) {
    const current = jobsById.get(job.id);
    if (!current || Date.parse(job.updatedAt) >= Date.parse(current.updatedAt) || finishesSameWorkflowRun(job, current)) {
      jobsById.set(job.id, job);
    }
  }
  return Array.from(jobsById.values());
}

export function latestFailedRenderJob(project: VideoProject) {
  const latest = project.jobs
    .filter((job) =>
      ["render_draft", "export_media", "exportMedia"].includes(job.kind),
    )
    .sort((left, right) => Date.parse(right.updatedAt) - Date.parse(left.updatedAt))[0];
  return latest?.status === "failed" ? latest : null;
}
