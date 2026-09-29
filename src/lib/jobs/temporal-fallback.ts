import {
  requestTemporalJobSummary,
  type ProjectAction,
  type ProjectJobSummary,
  type TemporalGenerateMediaBrief,
  type TemporalWorkerEnvironmentReport,
  type TemporalWorkflowStartRequest,
} from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";

export type TemporalWorkflowKind =
  | "generate_media"
  | "render_draft"
  | "transcribe_media"
  | "codex_edit"
  | "export_media";

export const temporalWorkflowDefinitions = {
  generate_media: {
    segment: "generate-media",
    workflowType: "VideoCreaterGenerateMediaWorkflow",
    activityTypes: ["BuildFalGenerationRequest", "RunMediaProviderGeneration"],
  },
  render_draft: {
    segment: "render-draft",
    workflowType: "VideoCreaterRenderDraftWorkflow",
    activityTypes: [
      "BuildRenderPlan",
      "RenderMedia",
      "ValidateRenderedMedia",
      "AttachRenderReport",
    ],
  },
  transcribe_media: {
    segment: "transcribe-media",
    workflowType: "VideoCreaterTranscribeMediaWorkflow",
    activityTypes: ["ProbeMedia", "RunTranscription", "StoreTranscript"],
  },
  codex_edit: {
    segment: "codex-edit",
    workflowType: "VideoCreaterCodexEditWorkflow",
    activityTypes: [
      "CollectProjectContext",
      "RequestCodexProposal",
      "ValidateProjectActions",
      "PersistAcceptedProposal",
      "AttachCodexEditFailure",
    ],
  },
  export_media: {
    segment: "export-media",
    workflowType: "VideoCreaterExportMediaWorkflow",
    activityTypes: [
      "BuildRenderPlan",
      "ValidateExportProfile",
      "RenderMedia",
      "ValidateRenderedMedia",
      "AttachRenderReport",
    ],
  },
} satisfies Record<
  TemporalWorkflowKind,
  { segment: string; workflowType: string; activityTypes: string[] }
>;

export function getFallbackTemporalWorkflowDefinition(kind: TemporalWorkflowKind) {
  return temporalWorkflowDefinitions[kind];
}

export function temporalIdSegment(value: string) {
  return (
    value
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "job"
  );
}

export function mockTemporalRunId(jobId: string) {
  const segment =
    jobId
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "workflow";
  return `mock-run-${segment}`;
}

export function isTemporalWorkerEnvironmentReport(
  value: unknown,
): value is TemporalWorkerEnvironmentReport {
  if (!value || typeof value !== "object") {
    return false;
  }

  const report = value as Partial<TemporalWorkerEnvironmentReport>;
  return (
    typeof report.ready === "boolean" &&
    typeof report.featureEnabled === "boolean" &&
    typeof report.taskQueue === "string" &&
    typeof report.localServiceTarget === "string" &&
    typeof report.localWebUiUrl === "string" &&
    typeof report.localDevCommand === "string" &&
    typeof report.workerRunCommand === "string" &&
    typeof report.featureName === "string" &&
    Array.isArray(report.tools)
  );
}

/**
 * Export start requests run only the activities of the selected profile (media render or
 * project bundle), so the recorded workflow metadata must describe the same activities.
 */
export function exportJobWithStartRequest(
  job: ProjectJobSummary,
  startRequest: TemporalWorkflowStartRequest,
): ProjectJobSummary {
  if (!job.workflow) return { ...job, startRequest };
  return {
    ...job,
    workflow: { ...job.workflow, activityTypes: [...startRequest.activityTypes] },
    startRequest,
  };
}

export async function buildTemporalJobSummary(
  kind: TemporalWorkflowKind,
  projectId: string,
  jobId: string,
  updatedAt: string,
): Promise<ProjectJobSummary> {
  try {
    return await requestTemporalJobSummary({
      kind,
      projectId,
      jobId,
      status: "queued",
      updatedAt,
    });
  } catch (error) {
    if (!isBackendUnavailableError(error)) {
      throw error;
    }

    const definition = temporalWorkflowDefinitions[kind];
    return {
      id: jobId,
      kind,
      status: "queued",
      updatedAt,
      workflow: {
        workflowId: `video-creater/${temporalIdSegment(projectId)}/${definition.segment}/${temporalIdSegment(
          jobId,
        )}`,
        workflowType: definition.workflowType,
        taskQueue: "video-creater-workflows",
        runId: null,
        activityTypes: definition.activityTypes,
      },
    };
  }
}

export function buildFallbackGenerateMediaStartRequest(
  input: {
    projectId: string;
    projectDir: string;
    assetId: string;
    jobId: string;
    mockMode: boolean;
  } & TemporalGenerateMediaBrief,
): TemporalWorkflowStartRequest {
  const { projectId, projectDir, assetId, jobId, mockMode, ...generationBrief } =
    input;
  const definition = temporalWorkflowDefinitions.generate_media;

  return {
    workflowId: `video-creater/${temporalIdSegment(projectId)}/${definition.segment}/${temporalIdSegment(
      jobId,
    )}`,
    workflowType: definition.workflowType,
    taskQueue: "video-creater-workflows",
    input: {
      projectId,
      projectDir,
      assetId,
      jobId,
      mockMode,
      ...generationBrief,
    },
    searchAttributes: {
      projectId,
      jobId,
      workflowKind: "generate_media",
    },
    activityTypes: definition.activityTypes,
    idReusePolicy: "rejectDuplicate",
  };
}

export function buildFallbackTranscribeMediaStartRequest(input: {
  projectId: string;
  projectDir: string;
  mediaId: string;
  jobId: string;
  languageMode: string;
}): TemporalWorkflowStartRequest {
  const definition = temporalWorkflowDefinitions.transcribe_media;

  return {
    workflowId: `video-creater/${temporalIdSegment(input.projectId)}/${definition.segment}/${temporalIdSegment(
      input.jobId,
    )}`,
    workflowType: definition.workflowType,
    taskQueue: "video-creater-workflows",
    input,
    searchAttributes: {
      projectId: input.projectId,
      jobId: input.jobId,
      workflowKind: "transcribe_media",
    },
    activityTypes: definition.activityTypes,
    idReusePolicy: "rejectDuplicate",
  };
}

export function buildFallbackTemporalStartResultAction(input: {
  job: ProjectJobSummary;
  runId: string;
  updatedAt: string;
}): ProjectAction {
  return {
    type: "updateJobStatus",
    jobId: input.job.id,
    status: "running",
    updatedAt: input.updatedAt,
    runId: input.runId,
  };
}
