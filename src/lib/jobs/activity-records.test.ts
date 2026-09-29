import { describe, expect, it } from "vitest";
import type {
  CodexEditProposal,
  ProjectExportArtifact,
  ProjectJobStatus,
  ProjectJobSummary,
  ProjectRenderReport,
  TemporalWorkflowStartRequest,
  VideoProject,
} from "@/lib/project";
import { sampleTimeline } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import {
  activeGenerationJobCount,
  activeJobCount,
  buildActivityJobRecords,
  canShowStartWorkflowAction,
  embeddedCodexProposal,
  formatJobKindSentenceCase,
  formatJobKindTitleCase,
  formatJobStatus,
  latestExportArtifact,
  latestFailedRenderJob,
  latestRenderReport,
  mergeProjectJobs,
  nonEmpty,
  projectAspectRatioLabel,
  projectFrameRateLabel,
  projectResolutionLabel,
  recentExportArtifacts,
  startRequestAudioValidationSummary,
  startRequestStatus,
  startRequestStringInput,
  startRequestValidationInput,
  startRequestVideoValidationSummary,
  validationString,
} from "@/lib/jobs/activity-records";

function projectWithActivity(): VideoProject {
  return {
    schemaVersion: 2,
    id: "activity-project",
    name: "Activity Project",
    createdAt: "2026-07-15T08:00:00Z",
    updatedAt: "2026-07-15T12:00:00Z",
    media: [],
    generatedAssets: [
      {
        schemaVersion: 1,
        id: "running-generation",
        kind: "generated",
        status: "running",
        name: "Hero variation",
        prompt: "Create a hero variation",
        model: { provider: "fal", id: "wan" },
        references: {
          mediaIds: [],
          firstFrameMediaId: null,
          lastFrameMediaId: null,
        },
        settings: {
          width: 1280,
          height: 720,
          durationSeconds: 4,
          fps: 24,
          aspectRatio: "16:9",
        },
        outputs: [],
        createdAt: "2026-07-15T12:00:00Z",
        parentAssetId: null,
        retryOfAssetId: null,
      },
    ],
    renderReports: [
      {
        schemaVersion: 1,
        id: "failed-render",
        status: "failed",
        outputPath: "renders/failed-render/output.webm",
        durationSeconds: 0,
        streams: { video: false, audio: false },
        checks: { playable: "failed" },
        artifacts: ["renders/failed-render/report.json"],
        logPath: "renders/failed-render/render.log",
        createdAt: "2026-07-15T11:00:00Z",
      },
    ],
    exportArtifacts: [
      {
        schemaVersion: 1,
        id: "completed-export-artifact",
        kind: "webm",
        format: "webm",
        path: "exports/final.webm",
        mimeType: "video/webm",
        jobId: "completed-export",
        createdAt: "2026-07-15T10:00:00Z",
      },
    ],
    transcripts: [],
    timeline: sampleTimeline,
    renderSettings: {
      width: 1920,
      height: 1080,
      fps: 24,
      loudnessLufs: -14,
      captions: "burn_in",
    },
    codexThreadId: null,
    jobs: [
      {
        id: "completed-export",
        kind: "export_media",
        status: "completed",
        updatedAt: "2026-07-15T10:00:00Z",
      },
      {
        id: "failed-render",
        kind: "render_draft",
        status: "failed",
        updatedAt: "2026-07-15T11:00:00Z",
      },
      {
        id: "running-generation",
        kind: "generate_media",
        status: "running",
        updatedAt: "2026-07-15T12:00:00Z",
      },
      {
        id: "queued-transcript",
        kind: "transcribe_media",
        status: "queued",
        updatedAt: "not-a-date",
      },
    ],
  };
}

const statuses: ProjectJobStatus[] = [
  "queued",
  "running",
  "progress",
  "blocked",
  "failed",
  "cancelled",
  "completed",
];

const jobKinds = [
  "generate_media",
  "render_draft",
  "transcribe_media",
  "codex_edit",
  "export_media",
  "exportMedia",
  "save_range",
  "nle_export",
  "__Mixed__CASE_kind_",
  "",
];

const validProposal: CodexEditProposal = {
  mediaId: "media-1",
  clips: [{ mediaId: "media-1", sourceIn: 0.2, sourceOut: 3.8, reason: "Exact hook" }],
  captions: [],
  overlays: [],
  hyperframes: [],
  gpuVisuals: [],
  projectActions: [],
  renderReview: {
    durationSeconds: 3.6,
    streamCheckRequired: true,
    captionAlignmentRequired: true,
    overlayTimingRequired: true,
    visualFrameEvidenceRequired: true,
    artifactPathsRequired: true,
    logReferenceRequired: true,
  },
};

function startRequest(overrides: Partial<TemporalWorkflowStartRequest> = {}): TemporalWorkflowStartRequest {
  return {
    workflowId: "video-creater/project/render-draft/job-1",
    workflowType: "VideoCreaterRenderDraftWorkflow",
    taskQueue: "video-creater-workflows",
    input: {},
    searchAttributes: {},
    activityTypes: ["BuildRenderPlan", "RenderMedia"],
    idReusePolicy: "rejectDuplicate",
    ...overrides,
  };
}

function job(overrides: Partial<ProjectJobSummary> = {}): ProjectJobSummary {
  return {
    id: "job-1",
    kind: "render_draft",
    status: "queued",
    updatedAt: "2026-09-13T00:00:00Z",
    ...overrides,
  };
}

const matchingWorkflow = {
  workflowId: "video-creater/project/render-draft/job-1",
  workflowType: "VideoCreaterRenderDraftWorkflow",
  taskQueue: "video-creater-workflows",
  runId: null,
  activityTypes: ["BuildRenderPlan", "RenderMedia"],
};

function jobsAcrossStatusesAndKinds(): ProjectJobSummary[] {
  return ["generate_media", "render_draft", "transcribe_media", "codex_edit", "export_media"].flatMap(
    (kind, kindIndex) =>
      statuses.map((status, statusIndex) =>
        job({
          id: `${kind}-${status}`,
          kind,
          status,
          updatedAt: `2026-09-13T0${kindIndex}:${String(statusIndex).padStart(2, "0")}:00Z`,
        }),
      ),
  );
}

function renderReport(id: string, createdAt: string): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id,
    status: "completed",
    outputPath: `renders/${id}/output.webm`,
    durationSeconds: 4,
    streams: { video: true, audio: true },
    checks: { playable: "passed" },
    artifacts: [],
    logPath: `renders/${id}/render.log`,
    createdAt,
  };
}

function exportArtifact(id: string, createdAt: string, jobId?: string | null): ProjectExportArtifact {
  return {
    schemaVersion: 1,
    id,
    kind: "webm",
    format: "webm",
    path: `exports/${id}.webm`,
    mimeType: "video/webm",
    ...(jobId === undefined ? {} : { jobId }),
    createdAt,
  };
}

describe("buildActivityJobRecords", () => {
  it("joins only persisted project evidence and orders every project job by update time", () => {
    const records = buildActivityJobRecords(projectWithActivity());

    expect(records.map(({ job }) => job.id)).toEqual([
      "running-generation",
      "failed-render",
      "completed-export",
      "queued-transcript",
    ]);
    expect(records[0]).toMatchObject({
      targetLabel: "Hero variation",
      proposalAvailable: false,
      outputPath: null,
      reportId: null,
      logPath: null,
    });
    expect(records[1]).toMatchObject({
      outputPath: "renders/failed-render/output.webm",
      reportId: "failed-render",
      logPath: "renders/failed-render/render.log",
    });
    expect(records[2]).toMatchObject({
      outputPath: "exports/final.webm",
      reportId: null,
      logPath: null,
    });
    expect(records[3]).toMatchObject({
      targetLabel: null,
      proposalAvailable: false,
      outputPath: null,
      reportId: null,
      logPath: null,
    });
  });
});

describe("export output paths", () => {
  it("prefers the saved export file over the render output", () => {
    const project = fixtureProject();
    project.jobs = [job({ id: "export-mp4H264-1", status: "completed" }), job({ id: "render-only", status: "completed" })];
    project.renderReports = [renderReport("export-mp4H264-1", "2026-09-17T00:00:00Z"), renderReport("render-only", "2026-09-17T00:00:00Z")];
    project.exportArtifacts = [{ ...exportArtifact("saved", "2026-09-17T00:00:00Z", "export-mp4H264-1"), path: "exports/Edison intro.mp4" }];

    const records = new Map(buildActivityJobRecords(project).map((record) => [record.job.id, record]));

    expect(records.get("export-mp4H264-1")?.outputPath).toBe("exports/Edison intro.mp4");
    expect(records.get("export-mp4H264-1")?.reportId).toBe("export-mp4H264-1");
    expect(records.get("render-only")?.outputPath).toBe("renders/render-only/output.webm");
  });
});

describe("activity record characterization", () => {
  it("builds records across statuses, kinds, proposals and persisted evidence", () => {
    const project = fixtureProject();
    project.jobs.push(...jobsAcrossStatusesAndKinds());
    project.jobs.push(
      job({
        id: "embedded-proposal",
        kind: "codex_edit",
        status: "completed",
        updatedAt: "2026-09-14T00:00:00Z",
        startRequest: startRequest({ input: { proposal: validProposal } }),
      }),
      job({
        id: "unresolved-proposal",
        kind: "codex_edit",
        status: "failed",
        updatedAt: "2026-09-14T00:01:00Z",
        startRequest: startRequest({ input: { proposalId: "proposal-without-a-resolver" } }),
      }),
      job({
        id: "blank-evidence",
        kind: "render_draft",
        status: "completed",
        updatedAt: "invalid",
      }),
    );
    project.generatedAssets.push({
      ...project.generatedAssets[0]!,
      id: "generate_media-running",
      name: "   ",
      outputs: [
        { ...project.generatedAssets[0]!.outputs[0]!, relativePath: "  " },
        { ...project.generatedAssets[0]!.outputs[0]!, relativePath: " generated/second.mp4 " },
      ],
    });
    project.renderReports.push(
      renderReport("render_draft-completed", "2026-09-13T01:00:00Z"),
      { ...renderReport("blank-evidence", "2026-09-13T01:00:00Z"), outputPath: " ", logPath: "" },
    );
    project.exportArtifacts = [
      exportArtifact("export-a", "2026-09-13T04:00:00Z", "export_media-completed"),
      exportArtifact("export-blank", "2026-09-13T04:00:00Z", " "),
      exportArtifact("export-render", "2026-09-13T04:00:00Z", "blank-evidence"),
    ];

    const records = buildActivityJobRecords(project);
    expect(
      records.map((record) => ({
        id: record.job.id,
        targetLabel: record.targetLabel,
        proposalAvailable: record.proposalAvailable,
        proposal: record.proposal === null ? null : record.proposal === validProposal,
        outputPath: record.outputPath,
        reportId: record.reportId,
        logPath: record.logPath,
      })),
    ).toMatchInlineSnapshot(`
      [
        {
          "id": "unresolved-proposal",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "embedded-proposal",
          "logPath": null,
          "outputPath": null,
          "proposal": true,
          "proposalAvailable": true,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "export_media-completed",
          "logPath": null,
          "outputPath": "exports/export-a.webm",
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "export_media-cancelled",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "export_media-failed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "export_media-blocked",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "export_media-progress",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "export_media-running",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "export_media-queued",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "codex_edit-completed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "codex_edit-cancelled",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "codex_edit-failed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "codex_edit-blocked",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "codex_edit-progress",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "codex_edit-running",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "codex_edit-queued",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "transcribe_media-completed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "transcribe_media-cancelled",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "transcribe_media-failed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "transcribe_media-blocked",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "transcribe_media-progress",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "transcribe_media-running",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "transcribe_media-queued",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "render_draft-completed",
          "logPath": "renders/render_draft-completed/render.log",
          "outputPath": "renders/render_draft-completed/output.webm",
          "proposal": null,
          "proposalAvailable": false,
          "reportId": "render_draft-completed",
          "targetLabel": null,
        },
        {
          "id": "render_draft-cancelled",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "render_draft-failed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "render_draft-blocked",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "render_draft-progress",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "render_draft-running",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "render_draft-queued",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "generate_media-completed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "generate_media-cancelled",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "generate_media-failed",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "generate_media-blocked",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "generate_media-progress",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "generate_media-running",
          "logPath": null,
          "outputPath": "generated/second.mp4",
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": "generate_media-running",
        },
        {
          "id": "generate_media-queued",
          "logPath": null,
          "outputPath": null,
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": null,
        },
        {
          "id": "sample-generated-shot",
          "logPath": null,
          "outputPath": "sample/generated/product-reveal.mp4",
          "proposal": null,
          "proposalAvailable": false,
          "reportId": null,
          "targetLabel": "Bundled Edison restoration",
        },
        {
          "id": "blank-evidence",
          "logPath": null,
          "outputPath": "exports/export-render.webm",
          "proposal": null,
          "proposalAvailable": false,
          "reportId": "blank-evidence",
          "targetLabel": null,
        },
      ]
    `);
  });

  it("normalizes non-empty strings", () => {
    expect(
      [null, undefined, "", "   ", " value ", "value", "\n\tx\n"].map((value) => nonEmpty(value)),
    ).toMatchInlineSnapshot(`
      [
        null,
        null,
        null,
        null,
        "value",
        "value",
        "x",
      ]
    `);
  });

  it("accepts only structurally complete embedded proposals", () => {
    const withProposal = (proposal: unknown) =>
      job({ startRequest: startRequest({ input: { proposal } }) });
    const cases: Array<[string, ProjectJobSummary]> = [
      ["no start request", job()],
      ["valid", withProposal(validProposal)],
      ["array", withProposal([validProposal])],
      ["string", withProposal("proposal")],
      ["missing media id", withProposal({ ...validProposal, mediaId: 1 })],
      ["bad clip", withProposal({ ...validProposal, clips: [{ sourceIn: "0", sourceOut: 1, reason: "x" }] })],
      ["null clip", withProposal({ ...validProposal, clips: [null] })],
      ["missing overlays", withProposal({ ...validProposal, overlays: undefined })],
      ["missing render review", withProposal({ ...validProposal, renderReview: null })],
      [
        "incomplete render review",
        withProposal({
          ...validProposal,
          renderReview: { ...validProposal.renderReview, logReferenceRequired: "yes" },
        }),
      ],
      ["unresolved proposal id", job({ startRequest: startRequest({ input: { proposalId: "p-1" } }) })],
    ];
    expect(
      cases.map(([label, candidate]) => [label, embeddedCodexProposal(candidate) !== null]),
    ).toMatchInlineSnapshot(`
      [
        [
          "no start request",
          false,
        ],
        [
          "valid",
          true,
        ],
        [
          "array",
          false,
        ],
        [
          "string",
          false,
        ],
        [
          "missing media id",
          false,
        ],
        [
          "bad clip",
          false,
        ],
        [
          "null clip",
          false,
        ],
        [
          "missing overlays",
          false,
        ],
        [
          "missing render review",
          false,
        ],
        [
          "incomplete render review",
          false,
        ],
        [
          "unresolved proposal id",
          false,
        ],
      ]
    `);
  });

  it("formats job kinds and statuses", () => {
    expect(
      jobKinds.map((kind) => [kind, formatJobKindTitleCase(kind), formatJobKindSentenceCase(kind)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "generate_media",
          "Generate Media",
          "Generate media",
        ],
        [
          "render_draft",
          "Render Draft",
          "Render draft",
        ],
        [
          "transcribe_media",
          "Transcribe Media",
          "Transcribe media",
        ],
        [
          "codex_edit",
          "Codex Edit",
          "Codex edit",
        ],
        [
          "export_media",
          "Export Media",
          "Export media",
        ],
        [
          "exportMedia",
          "Exportmedia",
          "Exportmedia",
        ],
        [
          "save_range",
          "Save Range",
          "Save range",
        ],
        [
          "nle_export",
          "Nle Export",
          "Nle export",
        ],
        [
          "__Mixed__CASE_kind_",
          "Mixed Case Kind",
          "Mixed case kind",
        ],
        [
          "",
          "",
          "",
        ],
      ]
    `);
    expect(statuses.map((status) => formatJobStatus(status))).toMatchInlineSnapshot(`
      [
        "Queued",
        "Running",
        "Progress",
        "Blocked",
        "Failed",
        "Cancelled",
        "Completed",
      ]
    `);
  });

  it("counts active jobs", () => {
    const project = fixtureProject();
    expect([activeJobCount(project), activeGenerationJobCount(project)]).toMatchInlineSnapshot(`
      [
        0,
        0,
      ]
    `);
    project.jobs.push(...jobsAcrossStatusesAndKinds());
    expect([activeJobCount(project), activeGenerationJobCount(project)]).toMatchInlineSnapshot(`
      [
        20,
        4,
      ]
    `);
  });

  it("selects latest render reports and export artifacts", () => {
    const project = fixtureProject();
    expect({
      latestRenderReport: latestRenderReport(project),
      latestExportArtifact: latestExportArtifact(project),
      recentExportArtifacts: recentExportArtifacts(project),
    }).toMatchInlineSnapshot(`
      {
        "latestExportArtifact": null,
        "latestRenderReport": null,
        "recentExportArtifacts": [],
      }
    `);

    project.renderReports.push(
      renderReport("report-new", "2026-09-13T02:00:00Z"),
      renderReport("report-old", "2026-09-13T01:00:00Z"),
    );
    project.exportArtifacts = [
      exportArtifact("export-2", "2026-09-13T02:00:00Z"),
      exportArtifact("export-4", "2026-09-13T04:00:00Z"),
      exportArtifact("export-1", "2026-09-13T01:00:00Z"),
      exportArtifact("export-3", "2026-09-13T03:00:00Z"),
    ];
    expect({
      latestRenderReport: latestRenderReport(project)?.id,
      latestExportArtifact: latestExportArtifact(project)?.id,
      recentExportArtifacts: recentExportArtifacts(project).map((artifact) => artifact.id),
      originalOrder: project.exportArtifacts.map((artifact) => artifact.id),
    }).toMatchInlineSnapshot(`
      {
        "latestExportArtifact": "export-3",
        "latestRenderReport": "report-old",
        "originalOrder": [
          "export-2",
          "export-4",
          "export-1",
          "export-3",
        ],
        "recentExportArtifacts": [
          "export-4",
          "export-3",
          "export-2",
        ],
      }
    `);

    project.exportArtifacts = [];
    expect([latestExportArtifact(project), recentExportArtifacts(project)]).toMatchInlineSnapshot(`
      [
        null,
        [],
      ]
    `);
  });

  it("classifies start requests against recorded workflow metadata", () => {
    const cases: Array<[string, ProjectJobSummary]> = [
      ["no start request", job({ workflow: matchingWorkflow })],
      ["null start request", job({ workflow: matchingWorkflow, startRequest: null })],
      ["no workflow", job({ startRequest: startRequest() })],
      ["matching", job({ workflow: matchingWorkflow, startRequest: startRequest() })],
      [
        "matching with run id",
        job({ workflow: { ...matchingWorkflow, runId: "run-1" }, startRequest: startRequest() }),
      ],
      [
        "workflow id mismatch",
        job({ workflow: matchingWorkflow, startRequest: startRequest({ workflowId: "other" }) }),
      ],
      [
        "workflow type mismatch",
        job({ workflow: matchingWorkflow, startRequest: startRequest({ workflowType: "Other" }) }),
      ],
      [
        "task queue mismatch",
        job({ workflow: matchingWorkflow, startRequest: startRequest({ taskQueue: "other" }) }),
      ],
      [
        "activity order mismatch",
        job({
          workflow: matchingWorkflow,
          startRequest: startRequest({ activityTypes: ["RenderMedia", "BuildRenderPlan"] }),
        }),
      ],
      [
        "activity length mismatch",
        job({ workflow: matchingWorkflow, startRequest: startRequest({ activityTypes: ["BuildRenderPlan"] }) }),
      ],
    ];
    expect(cases.map(([label, candidate]) => [label, startRequestStatus(candidate)])).toMatchInlineSnapshot(`
      [
        [
          "no start request",
          null,
        ],
        [
          "null start request",
          null,
        ],
        [
          "no workflow",
          "recorded",
        ],
        [
          "matching",
          "ready",
        ],
        [
          "matching with run id",
          "ready",
        ],
        [
          "workflow id mismatch",
          "mismatch",
        ],
        [
          "workflow type mismatch",
          "mismatch",
        ],
        [
          "task queue mismatch",
          "mismatch",
        ],
        [
          "activity order mismatch",
          "mismatch",
        ],
        [
          "activity length mismatch",
          "mismatch",
        ],
      ]
    `);
    expect(statuses.map((status) => [status, canShowStartWorkflowAction(status)])).toMatchInlineSnapshot(`
      [
        [
          "queued",
          true,
        ],
        [
          "running",
          false,
        ],
        [
          "progress",
          false,
        ],
        [
          "blocked",
          true,
        ],
        [
          "failed",
          false,
        ],
        [
          "cancelled",
          false,
        ],
        [
          "completed",
          false,
        ],
      ]
    `);
  });

  it("reads start request inputs and validation summaries", () => {
    const withInput = (input: Record<string, unknown>) => job({ startRequest: startRequest({ input }) });
    const cases: Array<[string, ProjectJobSummary]> = [
      ["no start request", job()],
      ["empty input", withInput({})],
      ["validation array", withInput({ validation: ["mp4"] })],
      ["validation string", withInput({ validation: "mp4" })],
      [
        "complete validation",
        withInput({
          outputPath: " exports/out.mp4 ",
          profile: "mp4H264",
          validation: {
            container: " mp4 ",
            videoCodec: "h264",
            mimeType: "video/mp4",
            audioCodec: " aac ",
          },
        }),
      ],
      [
        "partial validation",
        withInput({
          outputPath: "   ",
          profile: 42,
          validation: { container: "webm", videoCodec: "", mimeType: "video/webm", audioCodec: 7 },
        }),
      ],
    ];
    expect(
      cases.map(([label, candidate]) => ({
        label,
        outputPath: startRequestStringInput(candidate, "outputPath"),
        profile: startRequestStringInput(candidate, "profile"),
        validation: startRequestValidationInput(candidate),
        video: startRequestVideoValidationSummary(candidate),
        audio: startRequestAudioValidationSummary(candidate),
      })),
    ).toMatchInlineSnapshot(`
      [
        {
          "audio": null,
          "label": "no start request",
          "outputPath": null,
          "profile": null,
          "validation": null,
          "video": null,
        },
        {
          "audio": null,
          "label": "empty input",
          "outputPath": null,
          "profile": null,
          "validation": null,
          "video": null,
        },
        {
          "audio": null,
          "label": "validation array",
          "outputPath": null,
          "profile": null,
          "validation": null,
          "video": null,
        },
        {
          "audio": null,
          "label": "validation string",
          "outputPath": null,
          "profile": null,
          "validation": null,
          "video": null,
        },
        {
          "audio": "audio aac",
          "label": "complete validation",
          "outputPath": "exports/out.mp4",
          "profile": "mp4H264",
          "validation": {
            "audioCodec": " aac ",
            "container": " mp4 ",
            "mimeType": "video/mp4",
            "videoCodec": "h264",
          },
          "video": "mp4 / h264 / video/mp4",
        },
        {
          "audio": null,
          "label": "partial validation",
          "outputPath": null,
          "profile": null,
          "validation": {
            "audioCodec": 7,
            "container": "webm",
            "mimeType": "video/webm",
            "videoCodec": "",
          },
          "video": null,
        },
      ]
    `);
    expect(
      ["container", "missing", "blank", "number"].map((key) =>
        validationString({ container: " mp4 ", blank: "  ", number: 3 }, key),
      ),
    ).toMatchInlineSnapshot(`
      [
        "mp4",
        null,
        null,
        null,
      ]
    `);
  });

  it("labels project render settings", () => {
    const project = fixtureProject();
    const settings = [
      { width: 1920, height: 1080, fps: 24 },
      { width: 1080, height: 1920, fps: 29.97 },
      { width: 1001, height: 777, fps: 0 },
      { width: 0, height: 0, fps: Number.NaN },
    ];
    expect(
      settings.map((setting) => {
        const candidate = { ...project, renderSettings: { ...project.renderSettings, ...setting } };
        return [
          projectResolutionLabel(candidate),
          projectFrameRateLabel(candidate),
          projectAspectRatioLabel(candidate),
        ];
      }),
    ).toMatchInlineSnapshot(`
      [
        [
          "1920 x 1080",
          "24 fps",
          "16:9",
        ],
        [
          "1080 x 1920",
          "29.97 fps",
          "9:16",
        ],
        [
          "1001 x 777",
          "0 fps",
          "143:111",
        ],
        [
          "0 x 0",
          "NaN fps",
          "unknown",
        ],
      ]
    `);
  });

  it("merges project jobs by update time", () => {
    const current = [
      job({ id: "same-time", status: "queued", updatedAt: "2026-09-13T01:00:00Z" }),
      job({ id: "incoming-newer", status: "queued", updatedAt: "2026-09-13T01:00:00Z" }),
      job({ id: "incoming-older", status: "running", updatedAt: "2026-09-13T02:00:00Z" }),
      job({ id: "current-invalid", status: "queued", updatedAt: "invalid" }),
      job({ id: "incoming-invalid", status: "running", updatedAt: "2026-09-13T01:00:00Z" }),
      job({ id: "current-only", status: "blocked", updatedAt: "2026-09-13T01:00:00Z" }),
    ];
    const incoming = [
      job({ id: "incoming-only", status: "queued", updatedAt: "invalid" }),
      job({ id: "same-time", status: "completed", updatedAt: "2026-09-13T01:00:00Z" }),
      job({ id: "incoming-newer", status: "running", updatedAt: "2026-09-13T01:00:01Z" }),
      job({ id: "incoming-older", status: "failed", updatedAt: "2026-09-13T01:00:00Z" }),
      job({ id: "current-invalid", status: "completed", updatedAt: "2026-09-13T01:00:00Z" }),
      job({ id: "incoming-invalid", status: "failed", updatedAt: "invalid" }),
    ];
    expect(
      mergeProjectJobs(current, incoming).map((merged) => [merged.id, merged.status, merged.updatedAt]),
    ).toMatchInlineSnapshot(`
      [
        [
          "same-time",
          "completed",
          "2026-09-13T01:00:00Z",
        ],
        [
          "incoming-newer",
          "running",
          "2026-09-13T01:00:01Z",
        ],
        [
          "incoming-older",
          "running",
          "2026-09-13T02:00:00Z",
        ],
        [
          "current-invalid",
          "queued",
          "invalid",
        ],
        [
          "incoming-invalid",
          "running",
          "2026-09-13T01:00:00Z",
        ],
        [
          "current-only",
          "blocked",
          "2026-09-13T01:00:00Z",
        ],
        [
          "incoming-only",
          "queued",
          "invalid",
        ],
      ]
    `);
  });

  it("lets a workflow run's finished job replace the same run's running job stamped later", () => {
    // Temporal workflows stamp job updates with the workflow start time (whole seconds); the editor
    // records the started run a few milliseconds later, so its running copy has the newer time.
    const run = { ...matchingWorkflow, runId: "run-1" };
    const current = [
      job({ id: "temporal-export", kind: "export_media", status: "running", updatedAt: "2026-09-17T12:23:24.412Z", workflow: run }),
      job({ id: "retried-export", kind: "export_media", status: "running", updatedAt: "2026-09-17T12:23:24.412Z", workflow: { ...run, runId: "run-2" } }),
      job({ id: "in-process-render", status: "running", updatedAt: "2026-09-17T12:23:24.412Z" }),
    ];
    const incoming = [
      job({ id: "temporal-export", kind: "export_media", status: "completed", updatedAt: "2026-09-17T12:23:24Z", workflow: run }),
      job({ id: "retried-export", kind: "export_media", status: "failed", updatedAt: "2026-09-17T12:23:24Z", workflow: run }),
      job({ id: "in-process-render", status: "completed", updatedAt: "2026-09-17T12:23:24Z" }),
    ];
    expect(mergeProjectJobs(current, incoming).map((merged) => [merged.id, merged.status])).toEqual([
      ["temporal-export", "completed"],
      ["retried-export", "running"],
      ["in-process-render", "running"],
    ]);
  });

  it("finds the latest failed render job", () => {
    const project = fixtureProject();
    expect(latestFailedRenderJob(project)).toMatchInlineSnapshot(`null`);
    project.jobs.push(...jobsAcrossStatusesAndKinds());
    expect(latestFailedRenderJob(project)?.id).toMatchInlineSnapshot(`undefined`);
    project.jobs.push(
      job({ id: "legacy-export-failed", kind: "exportMedia", status: "failed", updatedAt: "2026-09-14T00:00:00Z" }),
    );
    expect(latestFailedRenderJob(project)?.id).toMatchInlineSnapshot(`"legacy-export-failed"`);
    project.jobs.push(
      job({ id: "render-completed-latest", kind: "render_draft", status: "completed", updatedAt: "2026-09-15T00:00:00Z" }),
    );
    expect(latestFailedRenderJob(project)).toMatchInlineSnapshot(`null`);
  });
});
