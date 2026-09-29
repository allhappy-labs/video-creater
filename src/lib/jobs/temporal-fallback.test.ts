import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ProjectJobSummary, TemporalWorkflowStartRequest } from "@/lib/project";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { backendRequest } from "@/lib/runtime/backend-client";

import {
  buildFallbackGenerateMediaStartRequest,
  buildFallbackTemporalStartResultAction,
  buildFallbackTranscribeMediaStartRequest,
  buildTemporalJobSummary,
  exportJobWithStartRequest,
  getFallbackTemporalWorkflowDefinition,
  isTemporalWorkerEnvironmentReport,
  mockTemporalRunId,
  temporalIdSegment,
  temporalWorkflowDefinitions,
  type TemporalWorkflowKind,
} from "@/lib/jobs/temporal-fallback";

vi.mock("@/lib/runtime/backend-client", () => ({
  backendMediaUrl: vi.fn((path: string) => `asset://${path}`),
  backendRequest: vi.fn(),
  backendListen: vi.fn(async () => () => undefined),
}));

const workflowKinds: TemporalWorkflowKind[] = [
  "generate_media",
  "render_draft",
  "transcribe_media",
  "codex_edit",
  "export_media",
];

describe("exportJobWithStartRequest", () => {
  it("records the activities that the profile-specific start request will run", () => {
    const job = {
      id: "export-1",
      kind: "export_media",
      status: "queued",
      updatedAt: "2026-09-13T00:00:00Z",
      workflow: {
        workflowId: "video-creater/project/export-media/export-1",
        workflowType: "VideoCreaterExportMediaWorkflow",
        taskQueue: "video-creater-workflows",
        runId: null,
        activityTypes: [
          "BuildRenderPlan",
          "ValidateExportProfile",
          "RenderMedia",
          "ValidateRenderedMedia",
          "AttachRenderReport",
          "WriteExportArtifact",
          "AttachExportReport",
        ],
      },
    } as ProjectJobSummary;
    const startRequest = {
      workflowId: "video-creater/project/export-media/export-1",
      workflowType: "VideoCreaterExportMediaWorkflow",
      taskQueue: "video-creater-workflows",
      idReusePolicy: "rejectDuplicate",
      searchAttributes: {},
      input: {},
      activityTypes: ["WriteExportArtifact", "AttachExportReport"],
    } as unknown as TemporalWorkflowStartRequest;

    const queued = exportJobWithStartRequest(job, startRequest);

    expect(queued.workflow?.activityTypes).toEqual(["WriteExportArtifact", "AttachExportReport"]);
    expect(queued.startRequest).toBe(startRequest);
    expect(job.workflow?.activityTypes).toHaveLength(7);
  });
});

describe("temporal fallback metadata", () => {
  it("uses canonical Temporal transcribe media fallback metadata", () => {
    expect(getFallbackTemporalWorkflowDefinition("transcribe_media")).toEqual({
      segment: "transcribe-media",
      workflowType: "VideoCreaterTranscribeMediaWorkflow",
      activityTypes: ["ProbeMedia", "RunTranscription", "StoreTranscript"],
    });
  });
});

describe("temporal fallback characterization", () => {
  beforeEach(() => {
    vi.mocked(backendRequest).mockReset();
  });

  it("defines fallback workflows for every kind", () => {
    expect(temporalWorkflowDefinitions).toMatchInlineSnapshot(`
      {
        "codex_edit": {
          "activityTypes": [
            "CollectProjectContext",
            "RequestCodexProposal",
            "ValidateProjectActions",
            "PersistAcceptedProposal",
            "AttachCodexEditFailure",
          ],
          "segment": "codex-edit",
          "workflowType": "VideoCreaterCodexEditWorkflow",
        },
        "export_media": {
          "activityTypes": [
            "BuildRenderPlan",
            "ValidateExportProfile",
            "RenderMedia",
            "ValidateRenderedMedia",
            "AttachRenderReport",
          ],
          "segment": "export-media",
          "workflowType": "VideoCreaterExportMediaWorkflow",
        },
        "generate_media": {
          "activityTypes": [
            "BuildFalGenerationRequest",
            "RunMediaProviderGeneration",
          ],
          "segment": "generate-media",
          "workflowType": "VideoCreaterGenerateMediaWorkflow",
        },
        "render_draft": {
          "activityTypes": [
            "BuildRenderPlan",
            "RenderMedia",
            "ValidateRenderedMedia",
            "AttachRenderReport",
          ],
          "segment": "render-draft",
          "workflowType": "VideoCreaterRenderDraftWorkflow",
        },
        "transcribe_media": {
          "activityTypes": [
            "ProbeMedia",
            "RunTranscription",
            "StoreTranscript",
          ],
          "segment": "transcribe-media",
          "workflowType": "VideoCreaterTranscribeMediaWorkflow",
        },
      }
    `);
    expect(workflowKinds.map((kind) => getFallbackTemporalWorkflowDefinition(kind))).toEqual(
      workflowKinds.map((kind) => temporalWorkflowDefinitions[kind]),
    );
  });

  it("slugs temporal id segments and mock run ids", () => {
    const values = ["export-1", "  Project Sample ", "--Clip__Name.MOV--", "***", "", "UPPER/lower"];
    expect(values.map((value) => [temporalIdSegment(value), mockTemporalRunId(value)])).toMatchInlineSnapshot(`
      [
        [
          "export-1",
          "mock-run-export-1",
        ],
        [
          "project-sample",
          "mock-run-project-sample",
        ],
        [
          "clip-name-mov",
          "mock-run-clip-name-mov",
        ],
        [
          "job",
          "mock-run-workflow",
        ],
        [
          "job",
          "mock-run-workflow",
        ],
        [
          "upper-lower",
          "mock-run-upper-lower",
        ],
      ]
    `);
  });

  it("recognizes Temporal worker environment reports", () => {
    const report = {
      ready: false,
      featureEnabled: true,
      taskQueue: "video-creater-workflows",
      localServiceTarget: "127.0.0.1:7233",
      localWebUiUrl: "http://127.0.0.1:8233",
      localDevCommand: "temporal server start-dev",
      workerRunCommand: "cargo run --bin worker",
      featureName: "temporal",
      tools: [],
    };
    const cases: Array<[string, unknown]> = [
      ["null", null],
      ["string", "report"],
      ["array", []],
      ["complete", report],
      ["missing tools", { ...report, tools: undefined }],
      ["tools object", { ...report, tools: {} }],
      ["ready string", { ...report, ready: "false" }],
      ["missing feature name", { ...report, featureName: undefined }],
    ];
    expect(
      cases.map(([label, value]) => [label, isTemporalWorkerEnvironmentReport(value)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "null",
          false,
        ],
        [
          "string",
          false,
        ],
        [
          "array",
          false,
        ],
        [
          "complete",
          true,
        ],
        [
          "missing tools",
          false,
        ],
        [
          "tools object",
          false,
        ],
        [
          "ready string",
          false,
        ],
        [
          "missing feature name",
          false,
        ],
      ]
    `);
  });

  it("copies start request activities into the recorded workflow", () => {
    const request: TemporalWorkflowStartRequest = {
      workflowId: "workflow-1",
      workflowType: "VideoCreaterExportMediaWorkflow",
      taskQueue: "video-creater-workflows",
      input: {},
      searchAttributes: {},
      activityTypes: ["WriteExportArtifact"],
      idReusePolicy: "rejectDuplicate",
    };
    const withoutWorkflow: ProjectJobSummary = {
      id: "export-2",
      kind: "export_media",
      status: "queued",
      updatedAt: "2026-09-13T00:00:00Z",
    };
    const nullWorkflow: ProjectJobSummary = { ...withoutWorkflow, workflow: null };
    const queued = exportJobWithStartRequest(withoutWorkflow, request);
    expect(exportJobWithStartRequest(nullWorkflow, request)).toMatchInlineSnapshot(`
      {
        "id": "export-2",
        "kind": "export_media",
        "startRequest": {
          "activityTypes": [
            "WriteExportArtifact",
          ],
          "idReusePolicy": "rejectDuplicate",
          "input": {},
          "searchAttributes": {},
          "taskQueue": "video-creater-workflows",
          "workflowId": "workflow-1",
          "workflowType": "VideoCreaterExportMediaWorkflow",
        },
        "status": "queued",
        "updatedAt": "2026-09-13T00:00:00Z",
        "workflow": null,
      }
    `);
    expect(queued).toMatchInlineSnapshot(`
      {
        "id": "export-2",
        "kind": "export_media",
        "startRequest": {
          "activityTypes": [
            "WriteExportArtifact",
          ],
          "idReusePolicy": "rejectDuplicate",
          "input": {},
          "searchAttributes": {},
          "taskQueue": "video-creater-workflows",
          "workflowId": "workflow-1",
          "workflowType": "VideoCreaterExportMediaWorkflow",
        },
        "status": "queued",
        "updatedAt": "2026-09-13T00:00:00Z",
      }
    `);
    expect(queued.startRequest).toBe(request);
    const withWorkflow = exportJobWithStartRequest(
      {
        ...withoutWorkflow,
        workflow: {
          workflowId: "workflow-1",
          workflowType: "VideoCreaterExportMediaWorkflow",
          taskQueue: "video-creater-workflows",
          runId: "run-1",
          activityTypes: [],
        },
      },
      request,
    );
    expect(withWorkflow.workflow).toMatchInlineSnapshot(`
      {
        "activityTypes": [
          "WriteExportArtifact",
        ],
        "runId": "run-1",
        "taskQueue": "video-creater-workflows",
        "workflowId": "workflow-1",
        "workflowType": "VideoCreaterExportMediaWorkflow",
      }
    `);
    expect(withWorkflow.workflow?.activityTypes).not.toBe(request.activityTypes);
  });

  it("requests the backend job summary when available", async () => {
    const summary: ProjectJobSummary = {
      id: "backend-job",
      kind: "render_draft",
      status: "queued",
      updatedAt: "2026-09-13T00:00:00Z",
    };
    vi.mocked(backendRequest).mockResolvedValue(summary);

    await expect(
      buildTemporalJobSummary("render_draft", "Project Sample", "Render Draft 1", "2026-09-13T00:00:00Z"),
    ).resolves.toBe(summary);
    expect(vi.mocked(backendRequest).mock.calls).toMatchInlineSnapshot(`
      [
        [
          "build_temporal_job_summary",
          {
            "jobId": "Render Draft 1",
            "kind": "render_draft",
            "projectId": "Project Sample",
            "status": "queued",
            "updatedAt": "2026-09-13T00:00:00Z",
          },
        ],
      ]
    `);
  });

  it("builds a local job summary when the backend is unavailable", async () => {
    vi.mocked(backendRequest).mockRejectedValue(new BackendUnavailableError());

    const summaries = await Promise.all(
      workflowKinds.map((kind) =>
        buildTemporalJobSummary(kind, "  Project Sample ", "Job__1", "2026-09-13T00:00:00Z"),
      ),
    );
    expect(summaries).toMatchInlineSnapshot(`
      [
        {
          "id": "Job__1",
          "kind": "generate_media",
          "status": "queued",
          "updatedAt": "2026-09-13T00:00:00Z",
          "workflow": {
            "activityTypes": [
              "BuildFalGenerationRequest",
              "RunMediaProviderGeneration",
            ],
            "runId": null,
            "taskQueue": "video-creater-workflows",
            "workflowId": "video-creater/project-sample/generate-media/job-1",
            "workflowType": "VideoCreaterGenerateMediaWorkflow",
          },
        },
        {
          "id": "Job__1",
          "kind": "render_draft",
          "status": "queued",
          "updatedAt": "2026-09-13T00:00:00Z",
          "workflow": {
            "activityTypes": [
              "BuildRenderPlan",
              "RenderMedia",
              "ValidateRenderedMedia",
              "AttachRenderReport",
            ],
            "runId": null,
            "taskQueue": "video-creater-workflows",
            "workflowId": "video-creater/project-sample/render-draft/job-1",
            "workflowType": "VideoCreaterRenderDraftWorkflow",
          },
        },
        {
          "id": "Job__1",
          "kind": "transcribe_media",
          "status": "queued",
          "updatedAt": "2026-09-13T00:00:00Z",
          "workflow": {
            "activityTypes": [
              "ProbeMedia",
              "RunTranscription",
              "StoreTranscript",
            ],
            "runId": null,
            "taskQueue": "video-creater-workflows",
            "workflowId": "video-creater/project-sample/transcribe-media/job-1",
            "workflowType": "VideoCreaterTranscribeMediaWorkflow",
          },
        },
        {
          "id": "Job__1",
          "kind": "codex_edit",
          "status": "queued",
          "updatedAt": "2026-09-13T00:00:00Z",
          "workflow": {
            "activityTypes": [
              "CollectProjectContext",
              "RequestCodexProposal",
              "ValidateProjectActions",
              "PersistAcceptedProposal",
              "AttachCodexEditFailure",
            ],
            "runId": null,
            "taskQueue": "video-creater-workflows",
            "workflowId": "video-creater/project-sample/codex-edit/job-1",
            "workflowType": "VideoCreaterCodexEditWorkflow",
          },
        },
        {
          "id": "Job__1",
          "kind": "export_media",
          "status": "queued",
          "updatedAt": "2026-09-13T00:00:00Z",
          "workflow": {
            "activityTypes": [
              "BuildRenderPlan",
              "ValidateExportProfile",
              "RenderMedia",
              "ValidateRenderedMedia",
              "AttachRenderReport",
            ],
            "runId": null,
            "taskQueue": "video-creater-workflows",
            "workflowId": "video-creater/project-sample/export-media/job-1",
            "workflowType": "VideoCreaterExportMediaWorkflow",
          },
        },
      ]
    `);
    expect(summaries[0]?.workflow?.activityTypes).toBe(temporalWorkflowDefinitions.generate_media.activityTypes);
  });

  it("rethrows backend failures other than unavailability", async () => {
    const failure = new Error("backend exploded");
    vi.mocked(backendRequest).mockRejectedValue(failure);

    await expect(
      buildTemporalJobSummary("codex_edit", "project", "job", "2026-09-13T00:00:00Z"),
    ).rejects.toBe(failure);

    vi.mocked(backendRequest).mockRejectedValue({ code: "backend_unavailable" });
    await expect(
      buildTemporalJobSummary("codex_edit", "project", "job", "2026-09-13T00:00:00Z"),
    ).resolves.toMatchInlineSnapshot(`
      {
        "id": "job",
        "kind": "codex_edit",
        "status": "queued",
        "updatedAt": "2026-09-13T00:00:00Z",
        "workflow": {
          "activityTypes": [
            "CollectProjectContext",
            "RequestCodexProposal",
            "ValidateProjectActions",
            "PersistAcceptedProposal",
            "AttachCodexEditFailure",
          ],
          "runId": null,
          "taskQueue": "video-creater-workflows",
          "workflowId": "video-creater/project/codex-edit/job",
          "workflowType": "VideoCreaterCodexEditWorkflow",
        },
      }
    `);
  });

  it("builds fallback generate and transcribe start requests", () => {
    expect(
      buildFallbackGenerateMediaStartRequest({
        projectId: "Project Sample",
        projectDir: "/tmp/project",
        assetId: "asset-1",
        jobId: "Generate Job 1",
        mockMode: true,
        name: "Hero",
        prompt: "A hero shot",
        placementIntent: "timeline",
        model: { provider: "fal", id: "wan" },
      }),
    ).toMatchInlineSnapshot(`
      {
        "activityTypes": [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
        "idReusePolicy": "rejectDuplicate",
        "input": {
          "assetId": "asset-1",
          "jobId": "Generate Job 1",
          "mockMode": true,
          "model": {
            "id": "wan",
            "provider": "fal",
          },
          "name": "Hero",
          "placementIntent": "timeline",
          "projectDir": "/tmp/project",
          "projectId": "Project Sample",
          "prompt": "A hero shot",
        },
        "searchAttributes": {
          "jobId": "Generate Job 1",
          "projectId": "Project Sample",
          "workflowKind": "generate_media",
        },
        "taskQueue": "video-creater-workflows",
        "workflowId": "video-creater/project-sample/generate-media/generate-job-1",
        "workflowType": "VideoCreaterGenerateMediaWorkflow",
      }
    `);
    expect(
      buildFallbackGenerateMediaStartRequest({
        projectId: "***",
        projectDir: "",
        assetId: "",
        jobId: "",
        mockMode: false,
      }),
    ).toMatchInlineSnapshot(`
      {
        "activityTypes": [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
        "idReusePolicy": "rejectDuplicate",
        "input": {
          "assetId": "",
          "jobId": "",
          "mockMode": false,
          "projectDir": "",
          "projectId": "***",
        },
        "searchAttributes": {
          "jobId": "",
          "projectId": "***",
          "workflowKind": "generate_media",
        },
        "taskQueue": "video-creater-workflows",
        "workflowId": "video-creater/job/generate-media/job",
        "workflowType": "VideoCreaterGenerateMediaWorkflow",
      }
    `);
    const transcribeInput = {
      projectId: "Project Sample",
      projectDir: "/tmp/project",
      mediaId: "media-1",
      jobId: "transcribe-media-1",
      languageMode: "auto",
    };
    const transcribe = buildFallbackTranscribeMediaStartRequest(transcribeInput);
    expect(transcribe).toMatchInlineSnapshot(`
      {
        "activityTypes": [
          "ProbeMedia",
          "RunTranscription",
          "StoreTranscript",
        ],
        "idReusePolicy": "rejectDuplicate",
        "input": {
          "jobId": "transcribe-media-1",
          "languageMode": "auto",
          "mediaId": "media-1",
          "projectDir": "/tmp/project",
          "projectId": "Project Sample",
        },
        "searchAttributes": {
          "jobId": "transcribe-media-1",
          "projectId": "Project Sample",
          "workflowKind": "transcribe_media",
        },
        "taskQueue": "video-creater-workflows",
        "workflowId": "video-creater/project-sample/transcribe-media/transcribe-media-1",
        "workflowType": "VideoCreaterTranscribeMediaWorkflow",
      }
    `);
    expect(transcribe.input).toBe(transcribeInput);
  });

  it("builds fallback start result actions", () => {
    expect(
      buildFallbackTemporalStartResultAction({
        job: {
          id: "job-1",
          kind: "render_draft",
          status: "queued",
          updatedAt: "2026-09-12T00:00:00Z",
        },
        runId: mockTemporalRunId("job-1"),
        updatedAt: "2026-09-13T00:00:00Z",
      }),
    ).toMatchInlineSnapshot(`
      {
        "jobId": "job-1",
        "runId": "mock-run-job-1",
        "status": "running",
        "type": "updateJobStatus",
        "updatedAt": "2026-09-13T00:00:00Z",
      }
    `);
  });
});
