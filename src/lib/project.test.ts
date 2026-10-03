import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  applyProjectActionLocally,
  applyTimelinePatchLocally,
  applyProjectActionToProject,
  applyProjectActionToSplitProjectFolder,
  applyProjectActionsToSplitProjectFolder,
  updateProjectSettingsInSplitProjectFolder,
  completeMockGeneratedAssetInSplitProjectFolder,
  createMatteInSplitProjectFolder,
  exportNleXmlToSplitProjectFolder,
  exportPalmierProjectPackageToSplitProjectFolder,
  importMediaToProject,
  loadSplitProjectFromFolder,
  migrateSingleFileProjectToSplit,
  orderRecentProjectJobs,
  planProjectRippleTrim,
  prepareProjectPreview,
  projectNeedsCanonicalPreview,
  projectActionFromTimelinePatch,
  searchProjectMedia,
  saveSplitProjectToFolder,
  buildTemporalCodexEditStartRequest,
  buildTemporalGenerateMediaStartRequest,
  buildTemporalTranscribeMediaStartRequest,
  buildTemporalExportMediaStartRequest,
  buildTemporalExportNleXmlStartRequest,
  buildTemporalGenerateMediaFailureActions,
  cancelGenerateMediaInProcess,
  cancelCodexConversationEditForProject,
  cancelCodexVideoEditForProject,
  captureCanonicalPreviewFrameInSplitProjectFolder,
  buildTemporalStartResultAction,
  canonicalizeProjectActionEffects,
  getExportProfileAvailabilityReport,
  listGenerationModelCatalog,
  getTemporalWorkerEnvironmentReport,
  renderMediaToSplitProjectFolder,
  retryGeneratedAssetOutputDownloadInSplitProjectFolder,
  revealExportArtifactInSplitProjectFolder,
  startTemporalWorkflow,
  startCodexConversationEditForProject,
  startCodexVideoEditForProject,
  applyCodexConversationProposal,
  undoLatestCodexConversationEdit,
  loadAppServerConversationHistoryFromSplitProjectFolder,
  validateSplitProjectFolder,
  type AppServerConversationEntry,
  type CodexConversationEditProposal,
  type CodexConversationEditRequest,
  type CodexPreparedProposal,
  type PreparedPreviewFrameResult,
  type ProjectAction,
  type ProjectAgentApplyResult,
  type ProjectAgentUndoOutcome,
  type ProjectJobSummary,
  type VideoProject,
} from "./project";
import type { EditJobRequest } from "./edit";
import { sampleTimeline, type TimelineTrack } from "./timeline";
import { maintainTransitions } from "./timeline-ops/transition-maintenance";
import {
  clip,
  cutProject,
  projectWithCrossfade,
  trackOf,
  transition,
  VIDEO_TRACK,
  withSourceRange,
  withVideoItem,
} from "./timeline-ops/transition-fixtures";
import { requiredAt } from "../test-utils/required";

function requiredTimelineTrack(timeline: VideoProject["timeline"], label: string) {
  return requiredAt(timeline.tracks, 0, `${label} track`);
}

function requiredTimelineItem(timeline: VideoProject["timeline"], label: string) {
  return requiredAt(requiredTimelineTrack(timeline, label).items, 0, `${label} item`);
}

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: vi.fn(),
}));

const project: VideoProject = {
  schemaVersion: 1,
  id: "project-1",
  name: "Project",
  createdAt: "2026-06-13T00:00:00Z",
  updatedAt: "2026-06-13T00:00:00Z",
  media: [],
  generatedAssets: [],
  renderReports: [],
  transcripts: [],
  timeline: { durationSeconds: 0, tracks: [] },
  renderSettings: {
    width: 1920,
    height: 1080,
    fps: 24,
    loudnessLufs: -14,
    captions: "burn_in",
  },
  codexThreadId: null,
  jobs: [],
};

const editRequest: EditJobRequest = {
  mediaId: "media-1",
  preset: "trailer_cut",
  prompt: "Make a tight edit.",
  targetDurationSeconds: 45,
  languageMode: "en",
  captionStyle: "bold",
  createdAt: "2026-06-23T00:00:00Z",
};

describe("project command adapters", () => {
  beforeEach(async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockReset();
  });

  it("calls the Rust media import command with project dir, project, and source paths", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      project,
      imported: [],
      skipped: [],
    });

    await importMediaToProject({
      projectDir: "/tmp/project",
      project,
      sourcePaths: ["/tmp/source.mp4"],
    });

    expect(invoke).toHaveBeenCalledWith("import_media_to_project", {
      projectDir: "/tmp/project",
      project,
      sourcePaths: ["/tmp/source.mp4"],
    });
  });

  it("forwards saved range names to the Rust media import command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({ project, imported: [], skipped: [] });

    await importMediaToProject({
      projectDir: "/p",
      project,
      sourcePaths: ["/p/renders/x/output.webm"],
      names: { "/p/renders/x/output.webm": "Edison 00:04–00:09" },
    });

    expect(invoke).toHaveBeenCalledWith("import_media_to_project", {
      projectDir: "/p",
      project,
      sourcePaths: ["/p/renders/x/output.webm"],
      names: { "/p/renders/x/output.webm": "Edison 00:04–00:09" },
    });
  });

  it("calls the Rust matte command with the selected aspect and folder", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({ project, media: project.media[0] });

    await createMatteInSplitProjectFolder({
      projectDir: "/tmp/project",
      request: { hex: "#112233", aspectRatio: "16:9", folderId: "folder-1" },
    });

    expect(invoke).toHaveBeenCalledWith("create_matte_in_split_project_folder", {
      projectDir: "/tmp/project",
      request: { hex: "#112233", aspectRatio: "16:9", folderId: "folder-1" },
    });
  });

  it("calls the Rust split project save command with project dir and project", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      manifestPath: "/tmp/project/video-creater.project.json",
      writtenFiles: [],
      removedFiles: [],
    });

    await saveSplitProjectToFolder({
      projectDir: "/tmp/project",
      project,
      expectedRevision: 0,
    });

    expect(invoke).toHaveBeenCalledWith("save_split_project_to_folder", {
      projectDir: "/tmp/project",
      project,
      expectedRevision: 0,
    });
  });

  it("calls the Rust split project load command with project dir", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue(project);

    await loadSplitProjectFromFolder({ projectDir: "/tmp/project" });

    expect(invoke).toHaveBeenCalledWith("load_split_project_from_folder", {
      projectDir: "/tmp/project",
    });
  });

  it("calls the Rust canonical preview preparation command with the in-memory project", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({ project, reports: [] });

    await prepareProjectPreview({ projectDir: "/tmp/project", project });

    expect(invoke).toHaveBeenCalledWith("prepare_project_preview", {
      projectDir: "/tmp/project",
      project,
    });
  });

  it("requires canonical frames for orphan shader and motion template items", () => {
    for (const [kind, properties] of [
      ["hyperframe_scene", { shaderBackgroundTemplateId: "shadertoy-octagrams-v1" }],
      ["overlay", { templateId: "kinetic-lower-third-v1" }],
    ] as const) {
      const visualProject: VideoProject = {
        ...project,
        timeline: { durationSeconds: 4, tracks: [{
          id: "graphics", name: "Graphics", kind, enabled: true, locked: false,
          items: [{ id: "visual", kind, startSeconds: 0, durationSeconds: 4,
            source: { type: "generated", artifactId: "orphan-template" }, label: "Visual", properties }],
        }] },
      };
      expect(projectNeedsCanonicalPreview(visualProject)).toBe(true);
      visualProject.timeline.tracks[0]!.enabled = false;
      expect(projectNeedsCanonicalPreview(visualProject)).toBe(false);
    }
  });

  it("detects only visual classes that require canonical prepared preview sources", () => {
    expect(projectNeedsCanonicalPreview(project)).toBe(false);
    const withLottie = {
      ...project,
      media: [
        {
          id: "lottie-1",
          relativePath: "media/badge.json",
          kind: "lottie" as const,
          durationSeconds: 2,
          width: 640,
          height: 360,
          fps: 30,
        },
      ],
      timeline: {
        durationSeconds: 2,
        tracks: [
          {
            id: "visuals",
            name: "Visuals",
            kind: "video" as const,
            locked: false,
            enabled: true,
            items: [
              {
                id: "badge",
                kind: "video_clip" as const,
                startSeconds: 0,
                durationSeconds: 2,
                source: { type: "media" as const, mediaId: "lottie-1" },
                label: "Badge",
                properties: {},
              },
            ],
          },
        ],
      },
    } satisfies VideoProject;
    expect(projectNeedsCanonicalPreview(withLottie)).toBe(true);
    expect(
      projectNeedsCanonicalPreview({
        ...withLottie,
        media: [{ ...requiredAt(withLottie.media, 0, "Lottie media"), kind: "video" }],
        timeline: {
          ...withLottie.timeline,
          tracks: [
            {
              ...requiredTimelineTrack(withLottie.timeline, "Lottie timeline"),
              items: [
                {
                  ...requiredTimelineItem(withLottie.timeline, "Lottie timeline"),
                  properties: { blendMode: "multiply" },
                },
              ],
            },
          ],
        },
      }),
    ).toBe(true);
  });

  it("needs canonical preview for reversed clips on enabled tracks", () => {
    const reversedOn = (kind: "video" | "audio", enabled: boolean): VideoProject => ({
      ...project,
      timeline: {
        durationSeconds: 2,
        tracks: [
          {
            id: kind,
            name: kind,
            kind,
            locked: false,
            enabled,
            items: [
              {
                id: "clip",
                kind: kind === "video" ? "video_clip" : "audio_clip",
                startSeconds: 0,
                durationSeconds: 2,
                source: { type: "media", mediaId: "media-1" },
                label: "Clip",
                properties: { reverse: true },
              },
            ],
          },
        ],
      },
    });
    expect(projectNeedsCanonicalPreview(reversedOn("video", true))).toBe(true);
    expect(projectNeedsCanonicalPreview(reversedOn("audio", true))).toBe(true);
    expect(projectNeedsCanonicalPreview(reversedOn("audio", false))).toBe(false);
  });

  it("calls the Rust split project validation command with project dir", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({ ok: true, issues: [] });

    await validateSplitProjectFolder({ projectDir: "/tmp/project" });

    expect(invoke).toHaveBeenCalledWith("validate_split_project_folder", {
      projectDir: "/tmp/project",
    });
  });

  it("calls the Rust project media search command with bounded search options", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      query: "launch day",
      limit: 10,
      visualStatus: "notInstalled",
      spokenStatus: "ready",
      groups: { spoken: [], visual: [], metadata: [], generated: [] },
      results: [],
      returned: 0,
    });

    await searchProjectMedia({
      projectDir: "/tmp/project",
      query: "launch day",
      limit: 10,
      scope: "both",
      mediaId: "media-1",
    });

    expect(invoke).toHaveBeenCalledWith("search_project_media", {
      projectDir: "/tmp/project",
      query: "launch day",
      limit: 10,
      scope: "both",
      mediaId: "media-1",
    });
  });

  it("calls the Rust split project migration command with project dir", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      manifestPath: "/tmp/project/video-creater.project.json",
      writtenFiles: ["/tmp/project/timeline.json"],
      removedFiles: [],
    });

    await migrateSingleFileProjectToSplit({ projectDir: "/tmp/project" });

    expect(invoke).toHaveBeenCalledWith(
      "migrate_single_file_project_to_split",
      {
        projectDir: "/tmp/project",
      },
    );
  });

  it("calls the Rust Temporal generate media start request command without credentials", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      workflowId: "video-creater/project-1/generate-media/generated-media-1",
      workflowType: "VideoCreaterGenerateMediaWorkflow",
      taskQueue: "video-creater-workflows",
      input: {
        projectId: "project-1",
        projectDir: "/tmp/project",
        assetId: "generated-media-1",
        jobId: "generated-media-1",
        mockMode: true,
      },
      searchAttributes: {
        projectId: "project-1",
        jobId: "generated-media-1",
        workflowKind: "generate_media",
      },
      activityTypes: [
        "BuildFalGenerationRequest",
        "RunMediaProviderGeneration",
      ],
      idReusePolicy: "rejectDuplicate",
    });

    const request = await buildTemporalGenerateMediaStartRequest({
      projectId: "project-1",
      projectDir: "/tmp/project",
      assetId: "generated-media-1",
      jobId: "generated-media-1",
      mockMode: true,
    });

    expect(invoke).toHaveBeenCalledWith(
      "build_temporal_generate_media_start_request",
      {
        projectId: "project-1",
        projectDir: "/tmp/project",
        assetId: "generated-media-1",
        jobId: "generated-media-1",
        mockMode: true,
      },
    );
    expect(request.input).not.toHaveProperty(["providerCredential", "EnvVar"].join(""));
    expect(request.input).not.toHaveProperty("providerCredential");
    expect(request.input).not.toHaveProperty("providerApiKey");
  });

  it("calls the Rust Temporal Codex edit start request command with replayable request data", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      workflowId: "video-creater/project-1/codex-edit/codex-edit-1",
      workflowType: "VideoCreaterCodexEditWorkflow",
      taskQueue: "video-creater-workflows",
      input: {
        projectId: "project-1",
        projectRoot: "/tmp/repo",
        projectDir: "/tmp/project",
        jobId: "codex-edit-1",
        request: editRequest,
      },
      searchAttributes: {
        projectId: "project-1",
        jobId: "codex-edit-1",
        workflowKind: "codex_edit",
      },
      activityTypes: [
        "CollectProjectContext",
        "RequestCodexProposal",
        "ValidateProjectActions",
        "PersistAcceptedProposal",
        "AttachCodexEditFailure",
      ],
      idReusePolicy: "rejectDuplicate",
    });

    const request = await buildTemporalCodexEditStartRequest({
      projectId: "project-1",
      projectRoot: "/tmp/repo",
      projectDir: "/tmp/project",
      jobId: "codex-edit-1",
      request: editRequest,
    });

    expect(invoke).toHaveBeenCalledWith(
      "build_temporal_codex_edit_start_request",
      {
        projectId: "project-1",
        projectRoot: "/tmp/repo",
        projectDir: "/tmp/project",
        jobId: "codex-edit-1",
        request: editRequest,
      },
    );
    expect(request.workflowType).toBe("VideoCreaterCodexEditWorkflow");
    expect(request.input.projectRoot).toBe("/tmp/repo");
    expect(request.input).not.toHaveProperty("proposal");
    expect(request.input).not.toHaveProperty("providerCredential");
  });

  it("calls the Rust Temporal transcribe media start request command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      workflowId: "video-creater/project-1/transcribe-media/transcribe-1",
      workflowType: "VideoCreaterTranscribeMediaWorkflow",
      taskQueue: "video-creater-workflows",
      input: {
        projectId: "project-1",
        projectDir: "/tmp/project",
        mediaId: "media-1",
        jobId: "transcribe-1",
        languageMode: "en",
      },
      searchAttributes: {
        projectId: "project-1",
        jobId: "transcribe-1",
        workflowKind: "transcribe_media",
      },
      activityTypes: ["ProbeMedia", "RunTranscription", "StoreTranscript"],
      idReusePolicy: "rejectDuplicate",
    });

    const request = await buildTemporalTranscribeMediaStartRequest({
      projectId: "project-1",
      projectDir: "/tmp/project",
      mediaId: "media-1",
      jobId: "transcribe-1",
      languageMode: "en",
    });

    expect(invoke).toHaveBeenCalledWith(
      "build_temporal_transcribe_media_start_request",
      {
        projectId: "project-1",
        projectDir: "/tmp/project",
        mediaId: "media-1",
        jobId: "transcribe-1",
        languageMode: "en",
      },
    );
    expect(request.workflowType).toBe("VideoCreaterTranscribeMediaWorkflow");
    expect(request.activityTypes).toEqual([
      "ProbeMedia",
      "RunTranscription",
      "StoreTranscript",
    ]);
    expect(request.searchAttributes.workflowKind).toBe("transcribe_media");
    expect(request.idReusePolicy).toBe("rejectDuplicate");
    expect(request.input).not.toHaveProperty(["providerCredential", "EnvVar"].join(""));
    expect(request.input).not.toHaveProperty("providerCredential");
    expect(request.input).not.toHaveProperty("providerApiKey");
  });

  it("calls the Rust Temporal workflow start command and returns unavailable runtime status", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const job: ProjectJobSummary = {
      id: "generated-media-1",
      kind: "generate_media",
      status: "queued",
      updatedAt: "2026-06-23T12:00:00Z",
      workflow: {
        workflowId: "video-creater/project-1/generate-media/generated-media-1",
        workflowType: "VideoCreaterGenerateMediaWorkflow",
        taskQueue: "video-creater-workflows",
        runId: null,
        activityTypes: [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
      },
      startRequest: {
        workflowId: "video-creater/project-1/generate-media/generated-media-1",
        workflowType: "VideoCreaterGenerateMediaWorkflow",
        taskQueue: "video-creater-workflows",
        input: {
          projectId: "project-1",
          projectDir: "/tmp/project",
          assetId: "generated-media-1",
          jobId: "generated-media-1",
          mockMode: false,
        },
        searchAttributes: {
          projectId: "project-1",
          jobId: "generated-media-1",
          workflowKind: "generate_media",
        },
        activityTypes: [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
        idReusePolicy: "rejectDuplicate",
      },
    };
    const startResult = {
      status: "unavailable",
      workflowId: "video-creater/project-1/generate-media/generated-media-1",
      workflowType: "VideoCreaterGenerateMediaWorkflow",
      taskQueue: "video-creater-workflows",
      runId: null,
      message: "Temporal runtime is unavailable in this build.",
    };
    vi.mocked(invoke).mockResolvedValue(startResult);

    const result = await startTemporalWorkflow({ job });

    expect(invoke).toHaveBeenCalledWith("start_temporal_workflow", { job });
    expect(result).toEqual(startResult);
  });

  it("calls the Rust Temporal worker environment report command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const report = {
      ready: false,
      featureEnabled: false,
      taskQueue: "video-creater-workflows",
      localServiceTarget: "http://localhost:7233",
      localWebUiUrl: "http://localhost:8233",
      localDevCommand: "temporal server start-dev",
      workerRunCommand:
        "cargo run --manifest-path src-tauri/Cargo.toml --features temporal-worker --bin video-creater-temporal-worker",
      featureName: "temporal-worker",
      tools: [
        {
          name: "protoc",
          available: false,
          path: null,
        },
        {
          name: "temporal",
          available: false,
          path: null,
        },
      ],
    };
    vi.mocked(invoke).mockResolvedValue(report);

    const result = await getTemporalWorkerEnvironmentReport();

    expect(invoke).toHaveBeenCalledWith(
      "get_temporal_worker_environment_report",
    );
    expect(result).toEqual(report);
  });

  it("calls the Rust export profile availability report command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const report = [
      {
        profile: "mp4H264",
        label: "MP4 / H.264",
        available: false,
        container: "mp4",
        extension: "mp4",
        mimeType: "video/mp4",
        videoCodec: "h264",
        audioCodec: "aac",
        requiredRuntime: ["approved-h264-encoder"],
        policyStatus: "missingRuntime",
        unavailableReason:
          "Missing approved local encoder runtime: approved-h264-encoder",
        qualityAvailability: { draft: false, final: false },
        qualityUnavailableReasons: {
          draft: "Missing approved local encoder runtime: approved-h264-encoder",
          final: "Missing approved local encoder runtime: approved-h264-encoder",
        },
      },
    ];
    vi.mocked(invoke).mockResolvedValue(report);

    const result = await getExportProfileAvailabilityReport();

    expect(invoke).toHaveBeenCalledWith(
      "get_export_profile_availability_report",
    );
    expect(result).toEqual(report);
  });

  it("calls the Rust generation model catalog command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const catalogPayload = {
      loaded: true,
      generationModels: [
        {
          provider: "catalog",
          id: "test-video",
          kind: "video",
          displayName: "Test Video",
        },
      ],
      providerCredentialsExposed: false,
    };
    vi.mocked(invoke).mockResolvedValue(catalogPayload);

    const result = await listGenerationModelCatalog();

    expect(invoke).toHaveBeenCalledWith("list_generation_model_catalog");
    expect(result).toEqual(catalogPayload);
  });

  it("calls the Rust native media render command with an export profile", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const result = {
      project,
      renderReport: {
        jobId: "export-mp4-1",
        summary: {
          status: "succeeded",
          durationSeconds: 4,
          outputPath: "renders/export-mp4-1/output.mp4",
        },
        command: { program: "gstreamer-ges", args: [] },
        stdout: "",
        stderr: "",
        streams: { video: true, audio: true },
        errors: [],
        artifacts: ["renders/export-mp4-1/output.mp4"],
        graphics: [],
        performance: null,
      },
      projectRenderReport: {
        schemaVersion: 1,
        id: "export-mp4-1",
        status: "completed",
        outputPath: "renders/export-mp4-1/output.mp4",
        durationSeconds: 4,
        streams: { video: true, audio: true },
        checks: {},
        artifacts: ["renders/export-mp4-1/output.mp4"],
        logPath: "renders/export-mp4-1/render.log",
        createdAt: "2026-07-03T00:00:00Z",
      },
      outputPath: "renders/export-mp4-1/output.mp4",
    };
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(
      renderMediaToSplitProjectFolder({
        projectDir: "/tmp/project",
        projectId: "project-1",
        profile: "mp4H264",
        quality: "final",
        width: 1920,
        height: 1080,
        jobId: "export-mp4-1",
        attemptId: "render-attempt/export-mp4-1",
        updatedAt: "2026-07-03T00:00:00Z",
      }),
    ).resolves.toEqual(result);

    expect(invoke).toHaveBeenCalledWith("render_media_to_split_project_folder", {
      projectDir: "/tmp/project",
      projectId: "project-1",
      profile: "mp4H264",
      quality: "final",
      width: 1920,
      height: 1080,
      jobId: "export-mp4-1",
      attemptId: "render-attempt/export-mp4-1",
      updatedAt: "2026-07-03T00:00:00Z",
    });
  });

  it("calls the Rust Temporal export media start request command without credentials", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const request = {
      workflowId: "video-creater/project-1/export-media/mp4-export-1",
      workflowType: "VideoCreaterExportMediaWorkflow",
      taskQueue: "video-creater-workflows",
      input: {
        projectId: "project-1",
        projectDir: "/tmp/project",
        jobId: "mp4-export-1",
        profile: "mp4H264",
        outputPath: "exports/project-1-h264.mp4",
        validation: {
          container: "mp4",
          extension: "mp4",
          mimeType: "video/mp4",
          videoCodec: "h264",
          audioCodec: "aac",
          requireVideoStream: true,
          requireAudioStreamWhenTimelineHasAudio: true,
        },
      },
      searchAttributes: {
        projectId: "project-1",
        jobId: "mp4-export-1",
        workflowKind: "export_media",
      },
      activityTypes: [
        "BuildRenderPlan",
        "ValidateExportProfile",
        "RenderMedia",
        "ValidateRenderedMedia",
        "AttachRenderReport",
      ],
      idReusePolicy: "rejectDuplicate",
    };
    vi.mocked(invoke).mockResolvedValue(request);

    const result = await buildTemporalExportMediaStartRequest({
      projectId: "project-1",
      projectDir: "/tmp/project",
      jobId: "h264-draft-1",
      profile: "mp4H264",
      quality: "draft",
      width: 1280,
      height: 720,
      outputPath: "exports/project-1-h264-draft.mp4",
    });

    expect(invoke).toHaveBeenCalledWith(
      "build_temporal_export_media_start_request",
      {
        projectId: "project-1",
        projectDir: "/tmp/project",
        jobId: "h264-draft-1",
        profile: "mp4H264",
        quality: "draft",
        width: 1280,
        height: 720,
        outputPath: "exports/project-1-h264-draft.mp4",
      },
    );
    expect(JSON.stringify(result).toLowerCase()).not.toContain("credential");
    expect(result).toEqual(request);
  });

  it("forwards the frame rate, Master tier and export folder to both export commands", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({});
    const exportSettings = {
      fps: 25,
      encodeTier: "master" as const,
      output: { fileName: "Edison intro", directory: "/tmp/vc-exports" },
    };

    await renderMediaToSplitProjectFolder({
      projectDir: "/tmp/project",
      projectId: "project-1",
      profile: "mp4H264",
      quality: "final",
      width: 1920,
      height: 1080,
      jobId: "export-mp4H264-1",
      attemptId: "render-attempt/export-mp4H264-1",
      updatedAt: "2026-09-17T00:00:00Z",
      ...exportSettings,
    });
    await buildTemporalExportMediaStartRequest({
      projectId: "project-1",
      projectDir: "/tmp/project",
      jobId: "export-mp4H264-2",
      profile: "mp4H264",
      quality: "final",
      width: 1920,
      height: 1080,
      outputPath: "exports/Edison intro.mp4",
      ...exportSettings,
    });

    expect(invoke).toHaveBeenCalledWith(
      "render_media_to_split_project_folder",
      expect.objectContaining(exportSettings),
    );
    expect(invoke).toHaveBeenCalledWith(
      "build_temporal_export_media_start_request",
      expect.objectContaining(exportSettings),
    );
  });

  it("calls the Rust Temporal NLE XML export start request command without credentials", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const request = {
      workflowId: "video-creater/project-1/export-nle-xml/nle-export-1",
      workflowType: "VideoCreaterExportNleXmlWorkflow",
      taskQueue: "video-creater-workflows",
      input: {
        projectId: "project-1",
        projectDir: "/tmp/project",
        jobId: "nle-export-1",
        format: "premiereXmeml",
        outputPath: "exports/project-1-premiere.xml",
      },
      searchAttributes: {
        projectId: "project-1",
        jobId: "nle-export-1",
        workflowKind: "export_nle_xml",
      },
      activityTypes: [
        "BuildNleXml",
        "ValidateNleXml",
        "WriteExportArtifact",
        "AttachExportReport",
      ],
      idReusePolicy: "rejectDuplicate",
    };
    vi.mocked(invoke).mockResolvedValue(request);

    const result = await buildTemporalExportNleXmlStartRequest({
      projectId: "project-1",
      projectDir: "/tmp/project",
      jobId: "nle-export-1",
      format: "premiereXmeml",
      outputPath: "exports/project-1-premiere.xml",
    });

    expect(invoke).toHaveBeenCalledWith(
      "build_temporal_export_nle_xml_start_request",
      {
        projectId: "project-1",
        projectDir: "/tmp/project",
        jobId: "nle-export-1",
        format: "premiereXmeml",
        outputPath: "exports/project-1-premiere.xml",
      },
    );
    expect(JSON.stringify(result).toLowerCase()).not.toContain("credential");
    expect(JSON.stringify(result).toLowerCase()).not.toContain("secret");
    expect(JSON.stringify(result).toLowerCase()).not.toContain("fal_key");
    expect(result).toEqual(request);
  });

  it("calls the Rust Temporal start result action command with recorded job metadata", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const job: ProjectJobSummary = {
      id: "generated-media-1",
      kind: "generate_media",
      status: "queued",
      updatedAt: "2026-06-23T12:00:00Z",
      workflow: {
        workflowId: "video-creater/project-1/generate-media/generated-media-1",
        workflowType: "VideoCreaterGenerateMediaWorkflow",
        taskQueue: "video-creater-workflows",
        runId: null,
        activityTypes: [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
      },
      startRequest: {
        workflowId: "video-creater/project-1/generate-media/generated-media-1",
        workflowType: "VideoCreaterGenerateMediaWorkflow",
        taskQueue: "video-creater-workflows",
        input: {
          projectId: "project-1",
          projectDir: "/tmp/project",
          assetId: "generated-media-1",
          jobId: "generated-media-1",
          mockMode: true,
        },
        searchAttributes: {
          projectId: "project-1",
          jobId: "generated-media-1",
          workflowKind: "generate_media",
        },
        activityTypes: [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
        idReusePolicy: "rejectDuplicate",
      },
    };
    const action = {
      type: "updateJobStatus",
      jobId: "generated-media-1",
      status: "running",
      updatedAt: "2026-06-23T12:01:00Z",
      runId: "temporal-run-1",
    } as const satisfies ProjectAction;
    vi.mocked(invoke).mockResolvedValue(action);

    const result = await buildTemporalStartResultAction({
      job,
      runId: "temporal-run-1",
      updatedAt: "2026-06-23T12:01:00Z",
    });

    expect(invoke).toHaveBeenCalledWith("build_temporal_start_result_action", {
      job,
      runId: "temporal-run-1",
      updatedAt: "2026-06-23T12:01:00Z",
    });
    expect(result).toEqual(action);
  });

  it("calls the Rust Temporal generated-media failure action command with recorded job metadata", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const job: ProjectJobSummary = {
      id: "generated-media-1",
      kind: "generate_media",
      status: "running",
      updatedAt: "2026-06-23T12:00:00Z",
      workflow: {
        workflowId: "video-creater/project-1/generate-media/generated-media-1",
        workflowType: "VideoCreaterGenerateMediaWorkflow",
        taskQueue: "video-creater-workflows",
        runId: "temporal-run-1",
        activityTypes: [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
      },
      startRequest: {
        workflowId: "video-creater/project-1/generate-media/generated-media-1",
        workflowType: "VideoCreaterGenerateMediaWorkflow",
        taskQueue: "video-creater-workflows",
        input: {
          projectId: "project-1",
          projectDir: "/tmp/project",
          assetId: "generated-media-1",
          jobId: "generated-media-1",
          mockMode: true,
        },
        searchAttributes: {
          projectId: "project-1",
          jobId: "generated-media-1",
          workflowKind: "generate_media",
        },
        activityTypes: [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
        idReusePolicy: "rejectDuplicate",
      },
    };
    const actions = [
      {
        type: "updateJobStatus",
        jobId: "generated-media-1",
        status: "failed",
        updatedAt: "2026-06-23T12:05:00Z",
        runId: "temporal-run-1",
      },
      {
        type: "updateGeneratedAssetStatus",
        assetId: "generated-media-1",
        status: "failed",
      },
    ] as const satisfies ProjectAction[];
    vi.mocked(invoke).mockResolvedValue(actions);

    const result = await buildTemporalGenerateMediaFailureActions({
      job,
      assetId: "generated-media-1",
      runId: "temporal-run-1",
      updatedAt: "2026-06-23T12:05:00Z",
    });

    expect(invoke).toHaveBeenCalledWith(
      "build_temporal_generate_media_failure_actions",
      {
        job,
        assetId: "generated-media-1",
        runId: "temporal-run-1",
        updatedAt: "2026-06-23T12:05:00Z",
      },
    );
    expect(result).toEqual(actions);
  });

  it("calls the in-process generation cancellation command with its atomic timestamp", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const cancellation = {
      outcome: "cancelled" as const,
      project,
    };
    vi.mocked(invoke).mockResolvedValue(cancellation);

    const result = await cancelGenerateMediaInProcess({
      projectDir: "/tmp/project",
      jobId: "generated-media-1",
      updatedAt: "2026-07-12T12:00:00Z",
    });

    expect(invoke).toHaveBeenCalledWith("cancel_generate_media_in_process", {
      projectDir: "/tmp/project",
      jobId: "generated-media-1",
      updatedAt: "2026-07-12T12:00:00Z",
    });
    expect(result).toEqual(cancellation);
  });

  it("calls the Rust NLE XML export command with project dir, format, and job metadata", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const job: ProjectJobSummary = {
      id: "nle-export-1",
      kind: "export_nle_xml",
      status: "completed",
      updatedAt: "2026-06-23T12:00:00Z",
      workflow: {
        workflowId: "video-creater/project-1/export-nle-xml/nle-export-1",
        workflowType: "VideoCreaterExportNleXmlWorkflow",
        taskQueue: "video-creater-workflows",
        runId: null,
        activityTypes: [
          "BuildNleXml",
          "ValidateNleXml",
          "WriteExportArtifact",
          "AttachExportReport",
        ],
      },
    };
    vi.mocked(invoke).mockResolvedValue({
      project: { ...project, jobs: [job] },
      exportPath: "/tmp/project/exports/project-1-premiere.xml",
      job,
    });

    await exportNleXmlToSplitProjectFolder({
      projectDir: "/tmp/project",
      format: "premiereXmeml",
      jobId: "nle-export-1",
      updatedAt: "2026-06-23T12:00:00Z",
    });

    expect(invoke).toHaveBeenCalledWith(
      "export_nle_xml_to_split_project_folder",
      {
        projectDir: "/tmp/project",
        format: "premiereXmeml",
        jobId: "nle-export-1",
        updatedAt: "2026-06-23T12:00:00Z",
      },
    );
  });

  it("calls the Rust Palmier project package export command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const input = {
      projectDir: "/tmp/project",
      jobId: "export-palmierProject-1",
      outputPath: "exports/project-1-palmierProject-1.palmier",
      updatedAt: "2026-09-13T12:00:00Z",
    };
    vi.mocked(invoke).mockResolvedValue({
      project,
      exportPath: "/tmp/project/exports/project-1-palmierProject-1.palmier",
      job: { id: input.jobId, kind: "export_media", status: "completed", updatedAt: input.updatedAt },
    });

    await exportPalmierProjectPackageToSplitProjectFolder(input);

    expect(invoke).toHaveBeenCalledWith(
      "export_palmier_project_package_to_split_project_folder",
      input,
    );
  });

  it("calls the Rust restricted export reveal command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue(undefined);

    await revealExportArtifactInSplitProjectFolder({
      projectDir: "/tmp/project",
      artifactPath: "renders/export-mp4-1/output.mp4",
    });

    expect(invoke).toHaveBeenCalledWith(
      "reveal_export_artifact_in_split_project_folder",
      { projectDir: "/tmp/project", artifactPath: "renders/export-mp4-1/output.mp4" },
    );
  });

  it("calls the Rust project action command with project and action", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "removeItems",
      itemIds: ["item-1"],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue({
      ...project,
      timeline: { durationSeconds: 0, tracks: [] },
    });

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts insert project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "insertItems",
      targetTrackId: "track-video",
      insertSeconds: 1.5,
      items: [
        {
          id: "item-inserted",
          kind: "video_clip",
          startSeconds: 0,
          durationSeconds: 2,
          source: { type: "media", mediaId: "media-1" },
          label: "Inserted clip",
          properties: {},
        },
      ],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts reorder project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "reorderItems",
      reorder: {
        targetTrackId: "track-video",
        itemIds: ["item-b", "item-a"],
        startSeconds: 1.25,
        gapSeconds: 0.2,
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("calls the Rust mock generated asset completion command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      project,
      report: {
        manifestPath: "/tmp/project/video-creater.project.json",
        writtenFiles: [],
        removedFiles: [],
      },
    });

    await completeMockGeneratedAssetInSplitProjectFolder({
      projectDir: "/tmp/project",
      assetId: "generated-shot-1",
      updatedAt: "2026-06-23T12:05:00Z",
    });

    expect(invoke).toHaveBeenCalledWith(
      "complete_mock_generated_asset_in_split_project_folder",
      {
        projectDir: "/tmp/project",
        assetId: "generated-shot-1",
        updatedAt: "2026-06-23T12:05:00Z",
      },
    );
  });

  it("calls the Rust mock generated asset completion command with a replacement target", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      project,
      report: {
        manifestPath: "/tmp/project/video-creater.project.json",
        writtenFiles: [],
        removedFiles: [],
      },
    });

    await completeMockGeneratedAssetInSplitProjectFolder({
      projectDir: "/tmp/project",
      assetId: "generated-shot-1",
      updatedAt: "2026-06-23T12:05:00Z",
      replacementItemId: "generated-clip-1",
    });

    expect(invoke).toHaveBeenCalledWith(
      "complete_mock_generated_asset_in_split_project_folder",
      {
        projectDir: "/tmp/project",
        assetId: "generated-shot-1",
        updatedAt: "2026-06-23T12:05:00Z",
        replacementItemId: "generated-clip-1",
      },
    );
  });

  it("calls the Rust generated output retry-download command", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      project,
      report: {
        manifestPath: "/tmp/project/video-creater.project.json",
        writtenFiles: [],
        removedFiles: [],
      },
    });

    await retryGeneratedAssetOutputDownloadInSplitProjectFolder({
      projectDir: "/tmp/project",
      assetId: "generated-shot-1",
      outputMediaId: "generated-shot-1-output",
    });

    expect(invoke).toHaveBeenCalledWith(
      "retry_generated_asset_output_download_in_split_project_folder",
      {
        projectDir: "/tmp/project",
        assetId: "generated-shot-1",
        outputMediaId: "generated-shot-1-output",
      },
    );
  });

  it("accepts workflow-backed job project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const job = {
      id: "render-draft-1",
      kind: "render_draft",
      status: "queued",
      updatedAt: "2026-06-23T12:00:00Z",
      workflow: {
        workflowId: "video-creater/project-1/render-draft/render-draft-1",
        workflowType: "VideoCreaterRenderDraftWorkflow",
        taskQueue: "video-creater-workflows",
        runId: null,
        activityTypes: [
          "BuildRenderPlan",
          "RenderMedia",
          "ValidateRenderedMedia",
          "AttachRenderReport",
        ],
      },
    } as const satisfies ProjectJobSummary;
    const action = {
      type: "recordJob",
      job,
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue({ ...project, jobs: [job] });

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts workflow job status updates in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateJobStatus",
      jobId: "render-draft-1",
      status: "running",
      updatedAt: "2026-06-23T12:01:00Z",
      runId: "temporal-run-1",
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts generated asset status updates in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateGeneratedAssetStatus",
      assetId: "generated-media-1",
      status: "running",
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts generated asset reference updates in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateGeneratedAssetReferences",
      assetId: "generated-media-1",
      references: {
        mediaIds: ["media-1"],
        firstFrameMediaId: "media-1",
        lastFrameMediaId: null,
        providerInputUrls: ["https://fal.media/uploads/source.png"],
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("translates timeline patches into equivalent project actions", () => {
    expect(
      projectActionFromTimelinePatch({
        type: "moveItem",
        itemId: "caption-1",
        targetTrackId: "track-captions",
        startSeconds: 1.65,
      }),
    ).toEqual({
      type: "moveItems",
      moves: [
        {
          itemId: "caption-1",
          targetTrackId: "track-captions",
          startSeconds: 1.65,
        },
      ],
    });

    expect(
      projectActionFromTimelinePatch({
        type: "resizeItem",
        itemId: "caption-1",
        durationSeconds: 1.1,
      }),
    ).toEqual({
      type: "resizeItems",
      resizes: [{ itemId: "caption-1", durationSeconds: 1.1 }],
    });

    expect(
      projectActionFromTimelinePatch({
        type: "editCaptionText",
        itemId: "caption-1",
        text: "Corrected caption",
      }),
    ).toEqual({
      type: "editCaptionText",
      itemId: "caption-1",
      text: "Corrected caption",
    });

    expect(
      projectActionFromTimelinePatch({
        type: "trimItem",
        itemId: "clip-1",
        startSeconds: 1.5,
        durationSeconds: 2.5,
        sourceIn: 3,
        sourceOut: 5.5,
      }),
    ).toEqual({
      type: "trimItems",
      trims: [
        {
          itemId: "clip-1",
          startSeconds: 1.5,
          durationSeconds: 2.5,
          sourceIn: 3,
          sourceOut: 5.5,
        },
      ],
    });
  });

  it("applies timeline patches locally for embedded projects", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;

    const movedProject = applyTimelinePatchLocally(embeddedProject, {
      type: "moveItem",
      itemId: "caption-2",
      targetTrackId: "track-captions",
      startSeconds: 0.15,
    });
    expect(
      movedProject.timeline.tracks
        .find((track) => track.id === "track-captions")
        ?.items.map((item) => `${item.id}:${item.startSeconds}`),
    ).toEqual(["caption-2:0.15", "caption-1:0.65"]);

    const resizedProject = applyTimelinePatchLocally(movedProject, {
      type: "resizeItem",
      itemId: "caption-1",
      durationSeconds: 1.1,
    });
    expect(
      resizedProject.timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "caption-1")?.durationSeconds,
    ).toBe(1.1);

    const trimmedProject = applyTimelinePatchLocally(resizedProject, {
      type: "trimItem",
      itemId: "item-1",
      startSeconds: 0.25,
      durationSeconds: 2.5,
      sourceIn: 1,
      sourceOut: 3.5,
    });
    const trimmedItem = trimmedProject.timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-1");
    expect(trimmedItem).toMatchObject({
      startSeconds: 0.25,
      durationSeconds: 2.5,
      properties: {
        sourceIn: 1,
        sourceOut: 3.5,
      },
    });

    const editedProject = applyTimelinePatchLocally(trimmedProject, {
      type: "editCaptionText",
      itemId: "caption-1",
      text: "Corrected caption",
    });
    expect(
      editedProject.timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "caption-1"),
    ).toMatchObject({
      label: "Corrected caption",
      source: { type: "text", text: "Corrected caption" },
      properties: { text: "Corrected caption", textEdited: true },
    });

    expect(
      embeddedProject.timeline.tracks
        .find((track) => track.id === "track-captions")
        ?.items.map((item) => `${item.id}:${item.startSeconds}`),
    ).toEqual(["caption-1:0.65", "caption-2:2.15"]);
  });

  it("removes unlocked tracks locally and rejects the whole batch like the Rust RemoveTracks action", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;
    const trackIds = (value: VideoProject) => value.timeline.tracks.map((track) => track.id);

    const removed = applyProjectActionLocally(embeddedProject, {
      type: "removeTracks",
      trackIds: ["track-scenes", "track-overlays", "track-scenes"],
    });
    expect(trackIds(removed)).toEqual(["track-video", "track-captions", "track-audio"]);

    const locked = applyProjectActionLocally(embeddedProject, {
      type: "setTrackLocked",
      trackId: "track-overlays",
      locked: true,
    });
    expect(
      applyProjectActionLocally(locked, {
        type: "removeTracks",
        trackIds: ["track-scenes", "track-overlays"],
      }),
    ).toBe(locked);
    expect(
      applyProjectActionLocally(embeddedProject, {
        type: "removeTracks",
        trackIds: ["track-scenes", "track-missing"],
      }),
    ).toBe(embeddedProject);
    expect(
      applyProjectActionLocally(embeddedProject, { type: "removeTracks", trackIds: [] }),
    ).toBe(embeddedProject);
    expect(
      applyProjectActionLocally(embeddedProject, { type: "removeTracks", trackIds: [" "] }),
    ).toBe(embeddedProject);
  });

  it("applies common project actions locally for embedded projects", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;

    const withTrackState = applyProjectActionLocally(embeddedProject, {
      type: "setTrackLocked",
      trackId: "track-video",
      locked: true,
    });
    expect(
      withTrackState.timeline.tracks.find((track) => track.id === "track-video")
        ?.locked,
    ).toBe(true);

    const withSyncLock = applyProjectActionLocally(withTrackState, {
      type: "setTrackSyncLocked",
      trackId: "track-audio",
      syncLocked: true,
    });
    expect(
      withSyncLock.timeline.tracks.find((track) => track.id === "track-audio")
        ?.syncLocked,
    ).toBe(true);

    const withReorderedTrack = applyProjectActionLocally(withSyncLock, {
      type: "reorderTrack",
      trackId: "track-overlays",
      targetTrackId: "track-scenes",
      placement: "before",
    });
    expect(withReorderedTrack.timeline.tracks.map((track) => track.id)).toEqual([
      "track-video",
      "track-overlays",
      "track-scenes",
      "track-captions",
      "track-audio",
    ]);

    const withAddedItem = applyProjectActionLocally(withReorderedTrack, {
      type: "addItems",
      targetTrackId: "track-overlays",
      items: [
        {
          id: "overlay-1",
          kind: "overlay",
          startSeconds: 1.25,
          durationSeconds: 1.5,
          source: { type: "text", text: "Overlay" },
          label: "Overlay",
          properties: {},
        },
      ],
    });
    expect(
      withAddedItem.timeline.tracks
        .find((track) => track.id === "track-overlays")
        ?.items.map((item) => item.id),
    ).toEqual(["overlay-1"]);
    expect(withAddedItem.timeline.durationSeconds).toBe(4);

    const withSplit = applyProjectActionLocally(withAddedItem, {
      type: "splitItems",
      splits: [{ itemId: "item-1", newItemId: "item-1-split", splitSeconds: 2.5 }],
    });
    expect(
      withSplit.timeline.tracks
        .find((track) => track.id === "track-video")
        ?.items.map((item) => ({
          id: item.id,
          startSeconds: item.startSeconds,
          durationSeconds: item.durationSeconds,
        })),
    ).toEqual([
      { id: "item-1", startSeconds: 0, durationSeconds: 2.5 },
      { id: "item-1-split", startSeconds: 2.5, durationSeconds: 1.5 },
    ]);

    const withEditedOverlay = applyProjectActionLocally(withSplit, {
      type: "updateTextOverlayItems",
      updates: [
        {
          itemId: "overlay-1",
          startSeconds: 1,
          durationSeconds: 2,
          text: "Updated overlay",
          visualTreatment: "lower corner label",
          motion: "fade in",
          safeZone: "inside 10% margins",
          avoid: "blocking faces",
        },
      ],
    });
    expect(
      withEditedOverlay.timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "overlay-1"),
    ).toMatchObject({
      label: "Updated overlay",
      startSeconds: 1,
      durationSeconds: 2,
      source: { type: "text", text: "Updated overlay" },
      properties: {
        text: "Updated overlay",
        visualTreatment: "lower corner label",
        motion: "fade in",
        safeZone: "inside 10% margins",
        avoid: "blocking faces",
        textEdited: true,
      },
    });

    const withKeyframes = applyProjectActionLocally(withEditedOverlay, {
      type: "setItemKeyframes",
      itemId: "overlay-1",
      property: "opacity",
      keyframes: [
        { atSeconds: 0, value: 0, easing: "linear" },
        { atSeconds: 1, value: 1, easing: "easeOut" },
      ],
    });
    expect(
      withKeyframes.timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "overlay-1")?.properties.keyframes,
    ).toEqual({
      opacity: [
        { atSeconds: 0, value: 0, easing: "linear" },
        { atSeconds: 1, value: 1, easing: "easeOut" },
      ],
    });

    const withRescaledKeyframes = applyProjectActionLocally(withKeyframes, {
      type: "resizeItems",
      resizes: [{ itemId: "overlay-1", durationSeconds: 4 }],
    });
    expect(
      withRescaledKeyframes.timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "overlay-1")?.properties.keyframes,
    ).toEqual({
      opacity: [
        { atSeconds: 0, value: 0, easing: "linear" },
        { atSeconds: 2, value: 1, easing: "easeOut" },
      ],
    });

    const withRemovedCaption = applyProjectActionLocally(withRescaledKeyframes, {
      type: "removeItems",
      itemIds: ["caption-2"],
    });
    expect(
      withRemovedCaption.timeline.tracks
        .flatMap((track) => track.items)
        .some((item) => item.id === "caption-2"),
    ).toBe(false);

    expect(
      embeddedProject.timeline.tracks.find((track) => track.id === "track-video")
        ?.items,
    ).toHaveLength(1);
  });

  it("keeps retimed source ranges, fades, and automation continuous in local split and overwrite fallbacks", () => {
    const automatedItem = {
      id: "retimed",
      kind: "video_clip" as const,
      startSeconds: 0,
      durationSeconds: 4,
      source: { type: "media" as const, mediaId: "media-1" },
      label: "Retimed",
      properties: {
        sourceIn: 10,
        sourceOut: 18,
        speed: 2,
        fadeInSeconds: 0.5,
        fadeOutSeconds: 0.75,
        keyframes: {
          opacity: [
            { atSeconds: 0, value: 0, easing: "easeInOut" as const },
            { atSeconds: 2, value: 1, easing: "linear" as const },
            { atSeconds: 4, value: 0, easing: "hold" as const },
          ],
        },
        effectParameterKeyframes: {
          "contrast-1": {
            amount: [
              { atSeconds: 0, value: 0.5, easing: "linear" as const },
              { atSeconds: 4, value: 1.5, easing: "linear" as const },
            ],
          },
        },
      },
    };
    const editingProject = {
      ...project,
      media: [{
        id: "media-1",
        relativePath: "media/source.mp4",
        kind: "video" as const,
        durationSeconds: 30,
        width: 1920,
        height: 1080,
        fps: 24,
      }],
      timeline: {
        durationSeconds: 4,
        tracks: [{
          id: "video",
          name: "Video",
          kind: "video" as const,
          locked: false,
          enabled: true,
          items: [automatedItem],
        }],
      },
    } satisfies VideoProject;

    const split = applyProjectActionLocally(editingProject, {
      type: "splitItems",
      splits: [{ itemId: "retimed", newItemId: "retimed-right", splitSeconds: 1.5 }],
    });
    const splitItems = requiredTimelineTrack(split.timeline, "split timeline").items;
    const left = requiredAt(splitItems, 0, "left split item");
    const right = requiredAt(splitItems, 1, "right split item");
    expect(left.properties).toMatchObject({ sourceIn: 10, sourceOut: 13, fadeInSeconds: 0.5 });
    expect(left.properties).not.toHaveProperty("fadeOutSeconds");
    expect(right.properties).toMatchObject({ sourceIn: 13, sourceOut: 18, fadeOutSeconds: 0.75 });
    expect(right.properties).not.toHaveProperty("fadeInSeconds");
    expect((left.properties.keyframes as { opacity: Array<{ value: number }> }).opacity.at(-1)?.value)
      .toBeCloseTo(0.84375);
    expect(requiredAt(
      (right.properties.keyframes as { opacity: Array<{ atSeconds: number; value: number }> }).opacity,
      0,
      "right opacity keyframe",
    ))
      .toMatchObject({ atSeconds: 0, value: 0.84375 });
    const contrastKeyframes = (right.properties.effectParameterKeyframes as Record<
      string,
      { amount: Array<{ value: number }> }
    >)["contrast-1"];
    expect(requiredAt(requiredAt(
      contrastKeyframes ? [contrastKeyframes] : [],
      0,
      "contrast keyframe group",
    ).amount, 0, "contrast amount keyframe").value)
      .toBeCloseTo(0.875);

    const overwritten = applyProjectActionLocally(editingProject, {
      type: "addItems",
      targetTrackId: "video",
      items: [{
        ...automatedItem,
        id: "incoming",
        startSeconds: 1,
        durationSeconds: 1,
        properties: { sourceIn: 0, sourceOut: 1 },
      }],
    });
    const overwrittenItems = requiredTimelineTrack(
      overwritten.timeline,
      "overwritten timeline",
    ).items;
    const overwrittenLeft = requiredAt(overwrittenItems, 0, "overwritten left item");
    const overwrittenRight = requiredAt(overwrittenItems, 2, "overwritten right item");
    expect(overwrittenLeft.properties.sourceOut).toBe(12);
    expect(overwrittenRight.properties).toMatchObject({ sourceIn: 14, sourceOut: 18 });
    expect(overwrittenLeft.properties).not.toHaveProperty("fadeOutSeconds");
    expect(overwrittenRight.properties).not.toHaveProperty("fadeInSeconds");
  });

  it("links and unlinks selected clips locally when the native bridge is unavailable", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;
    const linked = applyProjectActionLocally(embeddedProject, {
      type: "linkItems",
      itemIds: ["item-1", "caption-1"],
      linkGroupId: "link-local-1",
    });

    expect(
      linked.timeline.tracks
        .flatMap((track) => track.items)
        .filter((item) => ["item-1", "caption-1"].includes(item.id))
        .map((item) => item.properties.linkGroupId),
    ).toEqual(["link-local-1", "link-local-1"]);

    const unlinked = applyProjectActionLocally(linked, {
      type: "unlinkItems",
      itemIds: ["item-1", "caption-1"],
    });
    expect(
      unlinked.timeline.tracks
        .flatMap((track) => track.items)
        .filter((item) => ["item-1", "caption-1"].includes(item.id))
        .every((item) => item.properties.linkGroupId === undefined),
    ).toBe(true);
  });

  it("creates, switches, and renames alternate timelines locally", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;
    const alternate = applyProjectActionLocally(embeddedProject, {
      type: "createTimeline",
      timelineId: "alt-cut",
      name: "Alternate cut",
      duplicateActive: true,
    });
    expect(alternate.activeTimelineId).toBe("alt-cut");
    expect(alternate.timelines).toHaveLength(2);

    const renamed = applyProjectActionLocally(alternate, {
      type: "renameTimeline",
      timelineId: "alt-cut",
      name: "Social cut",
    });
    const main = applyProjectActionLocally(renamed, {
      type: "setActiveTimeline",
      timelineId: "main",
    });
    expect(main.activeTimelineId).toBe("main");
    expect(main.timelines?.find((timeline) => timeline.id === "alt-cut")?.name).toBe("Social cut");
    expect(main.timeline).toEqual(embeddedProject.timeline);
  });

  it("materializes the implicit timeline before renaming it locally", () => {
    const embeddedProject = { ...project, timeline: structuredClone(sampleTimeline) } satisfies VideoProject;
    delete embeddedProject.timelines;
    delete embeddedProject.activeTimelineId;
    const renamed = applyProjectActionLocally(embeddedProject, {
      type: "renameTimeline",
      timelineId: "main",
      name: "Rough cut",
    });
    expect(renamed.activeTimelineId).toBe("main");
    expect(renamed.timelines).toEqual([{ id: "main", name: "Rough cut", timeline: embeddedProject.timeline }]);
    expect(applyProjectActionLocally(embeddedProject, { type: "renameTimeline", timelineId: "other", name: "X" })).toBe(embeddedProject);
  });

  it("duplicates a selected background timeline and deletes the active timeline locally", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;
    const alternate = applyProjectActionLocally(embeddedProject, {
      type: "createTimeline",
      timelineId: "alt-cut",
      name: "Alternate cut",
      duplicateActive: false,
    });
    const main = applyProjectActionLocally(alternate, {
      type: "setActiveTimeline",
      timelineId: "main",
    });
    const duplicate = applyProjectActionLocally(main, {
      type: "createTimeline",
      timelineId: "alt-copy",
      name: "Copy of Alternate cut",
      duplicateActive: true,
      sourceTimelineId: "alt-cut",
    });
    expect(duplicate.timeline).toEqual(
      duplicate.timelines?.find((timeline) => timeline.id === "alt-cut")?.timeline,
    );
    expect(duplicate.timeline).not.toEqual(main.timeline);

    const deleted = applyProjectActionLocally(duplicate, {
      type: "deleteTimeline",
      timelineId: "alt-copy",
    });
    expect(deleted.activeTimelineId).toBe("main");
    expect(deleted.timeline).toEqual(embeddedProject.timeline);
    expect(deleted.timelines?.some((timeline) => timeline.id === "alt-copy")).toBe(false);
  });

  it("keeps the last or nested timeline when local deletion would break the project", () => {
    expect(
      applyProjectActionLocally(project, { type: "deleteTimeline", timelineId: "main" }),
    ).toBe(project);

    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;
    const alternate = applyProjectActionLocally(embeddedProject, {
      type: "createTimeline",
      timelineId: "alt-cut",
      name: "Alternate cut",
      duplicateActive: true,
    });
    const main = applyProjectActionLocally(alternate, {
      type: "setActiveTimeline",
      timelineId: "main",
    });
    const nested = applyProjectActionLocally(main, {
      type: "addItems",
      targetTrackId: "track-video",
      items: [{
        id: "nested-alt",
        kind: "video_clip",
        startSeconds: 10,
        durationSeconds: 1,
        source: { type: "timeline", timelineId: "alt-cut" },
        label: "Alternate cut",
        properties: {},
      }],
    });
    expect(
      applyProjectActionLocally(nested, { type: "deleteTimeline", timelineId: "alt-cut" }),
    ).toBe(nested);
  });

  it("decomposes an unstyled nested timeline locally without truncating child timing", () => {
    const mainTimeline = structuredClone(sampleTimeline);
    const alternateTimeline = structuredClone(sampleTimeline);
    requiredTimelineItem(alternateTimeline, "alternate timeline").properties = {
      sourceIn: 0,
      sourceOut: 4,
    };
    const wrapper = requiredTimelineItem(mainTimeline, "main timeline");
    wrapper.startSeconds = 1;
    wrapper.durationSeconds = 3;
    wrapper.source = { type: "timeline", timelineId: "alternate" };
    wrapper.properties = {};
    const embeddedProject = {
      ...project,
      timeline: mainTimeline,
      timelines: [
        { id: "main", name: "Timeline 1", timeline: mainTimeline },
        { id: "alternate", name: "Alternate", timeline: alternateTimeline },
      ],
      activeTimelineId: "main",
    } satisfies VideoProject;

    const decomposed = applyProjectActionLocally(embeddedProject, {
      type: "decomposeTimelineItem",
      itemId: "item-1",
    });

    expect(requiredTimelineTrack(decomposed.timeline, "decomposed timeline").items).toHaveLength(0);
    const child = decomposed.timeline.tracks
      .flatMap((track) => track.items)
      .find((item) => item.id === "item-1-item-1");
    expect(child).toMatchObject({ startSeconds: 1, durationSeconds: 3 });
    expect(child?.properties.sourceOut).toBe(3);
  });

  it("applies media folder project actions locally for embedded projects", () => {
    const projectMedia: VideoProject["media"] = [
      {
        id: "media-1",
        relativePath: "media/input.mp4",
        kind: "video",
        durationSeconds: 4,
        width: 1920,
        height: 1080,
        fps: 24,
      },
    ];
    const embeddedProject = {
      ...project,
      media: projectMedia,
      mediaFolders: [
        {
          id: "folder-generated",
          name: "Generated selects",
          parentId: null,
        },
      ],
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;

    const withAssignedMedia = applyProjectActionLocally(embeddedProject, {
      type: "assignMediaFolder",
      mediaId: "media-1",
      folderId: "folder-generated",
    });
    expect(withAssignedMedia.media.find((asset) => asset.id === "media-1")).toMatchObject({
      folderId: "folder-generated",
    });

    const withUnassignedMedia = applyProjectActionLocally(withAssignedMedia, {
      type: "assignMediaFolder",
      mediaId: "media-1",
      folderId: null,
    });
    expect(
      withUnassignedMedia.media.find((asset) => asset.id === "media-1")?.folderId,
    ).toBeNull();

    const withCreatedFolder = applyProjectActionLocally(withUnassignedMedia, {
      type: "createMediaFolder",
      folder: {
        id: "folder-broll",
        name: "B-roll",
        parentId: null,
      },
    });
    expect(withCreatedFolder.mediaFolders?.map((folder) => folder.id)).toEqual([
      "folder-generated",
      "folder-broll",
    ]);

    const withRenamedFolder = applyProjectActionLocally(withCreatedFolder, {
      type: "renameMediaFolder",
      folderId: "folder-generated",
      name: "Final selects",
    });
    expect(
      withRenamedFolder.mediaFolders?.find((folder) => folder.id === "folder-generated")
        ?.name,
    ).toBe("Final selects");

    const withDeletedFolder = applyProjectActionLocally(withRenamedFolder, {
      type: "deleteMediaFolder",
      folderId: "folder-generated",
    });
    expect(
      withDeletedFolder.mediaFolders?.some((folder) => folder.id === "folder-generated"),
    ).toBe(false);
    expect(withDeletedFolder.media.find((asset) => asset.id === "media-1")?.folderId).toBeNull();

    expect(
      "folderId" in (embeddedProject.media[0] ?? {})
        ? embeddedProject.media[0]?.folderId
        : undefined,
    ).toBeUndefined();
    expect(embeddedProject.mediaFolders?.[0]?.name).toBe("Generated selects");
  });

  it("applies generated asset workflow actions locally for browser fallbacks", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;
    const job = {
      id: "generated-shot-1",
      kind: "generate_media",
      status: "queued",
      updatedAt: "2026-06-27T12:00:00Z",
      workflow: {
        workflowId: "video-creater/project-1/generate-media/generated-shot-1",
        workflowType: "VideoCreaterGenerateMediaWorkflow",
        taskQueue: "video-creater-workflows",
        runId: null,
        activityTypes: [
          "BuildFalGenerationRequest",
          "RunMediaProviderGeneration",
        ],
      },
      startRequest: null,
    } as const satisfies ProjectJobSummary;
    const asset = {
      id: "generated-shot-1",
      kind: "generated",
      status: "queued",
      prompt: "quick product flash test",
      model: {
        provider: "fal.ai",
        id: "fal-ai/wan-25-preview/text-to-video",
      },
      references: {
        mediaIds: ["media-1"],
        firstFrameMediaId: "media-1",
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
      createdAt: "2026-06-27T12:00:00Z",
      name: null,
      targetFolderId: null,
      placementIntent: "library",
      parentAssetId: null,
      retryOfAssetId: null,
    } as const satisfies Extract<ProjectAction, { type: "recordGeneratedAsset" }>["asset"];

    const withJob = applyProjectActionLocally(embeddedProject, {
      type: "recordJob",
      job,
    });
    const withAsset = applyProjectActionLocally(withJob, {
      type: "recordGeneratedAsset",
      asset,
    });
    const withRunningJob = applyProjectActionLocally(withAsset, {
      type: "updateJobStatus",
      jobId: "generated-shot-1",
      status: "running",
      updatedAt: "2026-06-27T12:00:05Z",
      runId: "mock-run-generated-shot-1",
    });
    const withRunningAsset = applyProjectActionLocally(withRunningJob, {
      type: "updateGeneratedAssetStatus",
      assetId: "generated-shot-1",
      status: "running",
    });
    const withUploadedReferences = applyProjectActionLocally(withRunningAsset, {
      type: "updateGeneratedAssetReferences",
      assetId: "generated-shot-1",
      references: {
        mediaIds: ["media-1"],
        firstFrameMediaId: "media-1",
        lastFrameMediaId: null,
        providerInputUrls: ["https://fal.media/uploads/source.png"],
      },
    });

    expect(withAsset.jobs).toHaveLength(1);
    expect(withAsset.generatedAssets).toHaveLength(1);
    expect(withRunningAsset.jobs[0]).toMatchObject({
      id: "generated-shot-1",
      status: "running",
      updatedAt: "2026-06-27T12:00:05Z",
      workflow: { runId: "mock-run-generated-shot-1" },
    });
    expect(requiredAt(withRunningAsset.generatedAssets, 0, "running generated asset")).toMatchObject({
      id: "generated-shot-1",
      status: "running",
      prompt: "quick product flash test",
      placementIntent: "library",
    });
    expect(requiredAt(
      withUploadedReferences.generatedAssets,
      0,
      "uploaded-reference generated asset",
    ).references).toEqual({
      mediaIds: ["media-1"],
      firstFrameMediaId: "media-1",
      lastFrameMediaId: null,
      providerInputUrls: ["https://fal.media/uploads/source.png"],
    });
  });

  it("applies canonical keyframe point mutations locally", () => {
    const embeddedProject = {
      ...project,
      timeline: structuredClone(sampleTimeline),
    } satisfies VideoProject;
    const seeded = applyProjectActionLocally(embeddedProject, {
      type: "setItemKeyframes",
      itemId: "item-1",
      property: "opacity",
      keyframes: [
        { atSeconds: 2, value: 0.2, easing: "smooth" },
        { atSeconds: 0, value: 0, easing: "linear" },
      ],
    });
    const upserted = applyProjectActionLocally(seeded, {
      type: "upsertItemKeyframe",
      itemId: "item-1",
      property: "opacity",
      keyframe: { atSeconds: 1, value: 0.5, easing: "smooth" },
    });
    const collided = applyProjectActionLocally(upserted, {
      type: "moveItemKeyframe",
      itemId: "item-1",
      property: "opacity",
      fromSeconds: 0,
      toSeconds: 1,
    });
    expect(
      collided.timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "item-1")?.properties.keyframes,
    ).toEqual({
      opacity: [
        { atSeconds: 1, value: 0, easing: "linear" },
        { atSeconds: 2, value: 0.2, easing: "easeInOut" },
      ],
    });

    const deleted = applyProjectActionLocally(collided, {
      type: "deleteItemKeyframe",
      itemId: "item-1",
      property: "opacity",
      atSeconds: 1,
    });
    expect(
      deleted.timeline.tracks
        .flatMap((track) => track.items)
        .find((item) => item.id === "item-1")?.properties.keyframes,
    ).toEqual({
      opacity: [{ atSeconds: 2, value: 0.2, easing: "easeInOut" }],
    });
    expect(() =>
      applyProjectActionLocally(deleted, {
        type: "deleteItemKeyframe",
        itemId: "item-1",
        property: "opacity",
        atSeconds: 9,
      }),
    ).toThrow("Keyframe was not found");
  });

  it("plans and applies linked ripple trims with sync-locked followers", () => {
    const clip = (id: string, kind: "video_clip" | "audio_clip", mediaId: string, start: number) => ({
      id,
      kind,
      startSeconds: start,
      durationSeconds: kind === "video_clip" && id === "video-after" ? 2 : 4,
      source: { type: "media" as const, mediaId },
      label: id,
      properties: {
        sourceIn: id.endsWith("after") ? 6 : 2,
        sourceOut: id.endsWith("after") ? (kind === "video_clip" ? 8 : 10) : 6,
        ...(id.endsWith("-1") ? { linkGroupId: "linked-av-1" } : {}),
      },
    });
    const rippleProject = {
      ...project,
      media: [
        { id: "video", relativePath: "video.mp4", kind: "video" as const, durationSeconds: 12, width: 1920, height: 1080, fps: 24, folderId: null },
        { id: "audio", relativePath: "audio.wav", kind: "audio" as const, durationSeconds: 12, width: null, height: null, fps: null, folderId: null },
      ],
      timeline: {
        durationSeconds: 8,
        tracks: [
          { id: "video-track", name: "Video", kind: "video" as const, locked: false, items: [clip("video-1", "video_clip", "video", 0), clip("video-after", "video_clip", "video", 4)] },
          { id: "audio-track", name: "Audio", kind: "audio" as const, locked: false, items: [clip("audio-1", "audio_clip", "audio", 0), clip("audio-after", "audio_clip", "audio", 4)] },
          { id: "caption-track", name: "Captions", kind: "caption" as const, locked: false, items: [{ id: "caption-after", kind: "caption" as const, startSeconds: 4, durationSeconds: 1, source: { type: "text" as const, text: "Caption" }, label: "Caption", properties: {} }] },
        ],
      },
    } satisfies VideoProject;
    const action = {
      type: "rippleTrimItem",
      itemId: "video-1",
      edge: "right",
      deltaSeconds: 1,
      propagateLinked: true,
      syncLockedTrackIds: ["caption-track"],
    } as const;
    const plan = planProjectRippleTrim(rippleProject, action);
    expect(plan).toMatchObject({
      durationDeltaSeconds: 1,
      affectedTrackIds: ["video-track", "audio-track", "caption-track"],
      shifts: [
        { itemId: "video-after", startSeconds: 5 },
        { itemId: "audio-after", startSeconds: 5 },
        { itemId: "caption-after", startSeconds: 5 },
      ],
    });
    const applied = applyProjectActionLocally(rippleProject, action);
    expect(
      applied.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === "audio-1"),
    ).toMatchObject({ durationSeconds: 5, properties: { sourceOut: 7 } });
  });

  it("calls the Rust split project action command with project dir and action", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "removeItems",
      itemIds: ["item-1"],
    } as const satisfies Parameters<
      typeof applyProjectActionToSplitProjectFolder
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue({
      project,
      report: {
        manifestPath: "/tmp/project/video-creater.project.json",
        writtenFiles: ["/tmp/project/timeline.json"],
        removedFiles: [],
      },
    });

    await applyProjectActionToSplitProjectFolder({
      projectDir: "/tmp/project",
      action,
    });

    expect(invoke).toHaveBeenCalledWith(
      "apply_project_action_to_split_project_folder",
      {
        projectDir: "/tmp/project",
        action,
      },
    );
  });

  it("updates project settings through one dedicated split-project action", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const renderSettings: VideoProject["renderSettings"] = {
      width: 3840,
      height: 2160,
      fps: 24,
      loudnessLufs: -16,
      captions: "mux",
    };
    vi.mocked(invoke).mockResolvedValue({
      project: { ...project, name: "Interview cut", renderSettings },
      report: {
        manifestPath: "/tmp/project/video-creater.project.json",
        writtenFiles: ["/tmp/project/video-creater.project.json"],
        removedFiles: [],
      },
    });

    const result = await updateProjectSettingsInSplitProjectFolder({
      projectDir: "/tmp/project",
      name: "Interview cut",
      renderSettings,
    });

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(
      "update_project_settings_in_split_project_folder",
      {
        projectDir: "/tmp/project",
        name: "Interview cut",
        renderSettings,
      },
    );
    expect(result.project.name).toBe("Interview cut");
  });

  it("calls the Rust split project batch action command with project dir and ordered actions", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const actions = [
      {
        type: "editCaptionText",
        itemId: "caption-1",
        text: "Corrected caption",
      },
      {
        type: "removeItems",
        itemIds: ["item-1"],
      },
    ] as const satisfies Parameters<
      typeof applyProjectActionsToSplitProjectFolder
    >[0]["actions"];
    vi.mocked(invoke).mockResolvedValue({
      project,
      report: {
        manifestPath: "/tmp/project/video-creater.project.json",
        writtenFiles: ["/tmp/project/timeline.json"],
        removedFiles: [],
      },
    });

    await applyProjectActionsToSplitProjectFolder({
      projectDir: "/tmp/project",
      actions,
    });

    expect(invoke).toHaveBeenCalledWith(
      "apply_project_actions_to_split_project_folder",
      {
        projectDir: "/tmp/project",
        actions,
      },
    );
  });

  it("calls the Codex video edit command and returns a typed proposal payload", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "editCaptionText",
      itemId: "caption-1",
      text: "Corrected caption",
    } as const satisfies ProjectAction;
    vi.mocked(invoke).mockResolvedValue({
      project,
      threadId: "thread-1",
      threadResponse: { thread: { id: "thread-1" } },
      turnResponse: { structuredOutput: true },
      proposal: {
        mediaId: "media-1",
        clips: [{ sourceIn: 1, sourceOut: 5, reason: "hook" }],
        captions: [],
        overlays: [],
        hyperframes: [],
        gpuVisuals: [],
        projectActions: [action],
        renderReview: {
          durationSeconds: 4,
          streamCheckRequired: true,
          captionAlignmentRequired: true,
          overlayTimingRequired: true,
          visualFrameEvidenceRequired: true,
          artifactPathsRequired: true,
          logReferenceRequired: true,
        },
      },
    });

    const result = await startCodexVideoEditForProject({
      projectRoot: "/tmp/repo",
      project,
      request: editRequest,
    });

    expect(invoke).toHaveBeenCalledWith("start_codex_video_edit_for_project", {
      projectRoot: "/tmp/repo",
      project,
      request: editRequest,
    });
    expect(result.proposal?.projectActions).toEqual([action]);
  });

  it("starts a preset-free Codex conversation edit and returns the proposal payload", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateAudioVolume",
      itemId: "audio-1",
      volumeDb: -2,
    } as const satisfies ProjectAction;
    const request = {
      prompt: "Balance the dialogue.",
      focus: {
        primaryMediaId: null,
        mediaIds: ["media-1"],
        timelineItemIds: ["audio-1"],
        timelineRange: { startSeconds: 2, endSeconds: 6 },
      },
      createdAt: "2026-07-25T00:00:00Z",
    } satisfies CodexConversationEditRequest;
    const preparedProposal = {
      actions: [action],
      actionIds: ["codex-action-1-0a1b2c3d4e5f"],
      risk: { level: "safe", reasons: [] },
      impact: {
        summary: "Changes 1 item; the timeline stays 10.0s.",
        beforeDurationSeconds: 10,
        afterDurationSeconds: 10,
        affectedItemIds: ["audio-1"],
        affectedRanges: [{ startSeconds: 0, endSeconds: 10 }],
        previewTimestamp: 0,
      },
    } satisfies CodexPreparedProposal;
    vi.mocked(invoke).mockResolvedValue({
      project,
      threadId: "thread-1",
      threadResponse: { thread: { id: "thread-1" } },
      turnResponse: { id: "turn-1", status: "completed" },
      proposal: {
        summary: "Lowered the interview clip by 2 dB.",
        edl: [],
        projectActions: [action],
        renderReview: null,
      },
      preparedProposal,
      proposalValidationIssues: [],
    });

    const result = await startCodexConversationEditForProject({
      projectDir: "/tmp/project",
      project,
      request,
    });

    expect(invoke).toHaveBeenCalledWith("start_codex_conversation_edit_for_project", {
      projectDir: "/tmp/project",
      project,
      request,
    });
    expect(request).not.toHaveProperty("preset");
    expect(request).not.toHaveProperty("targetDurationSeconds");
    expect(result.proposal?.summary).toBe("Lowered the interview clip by 2 dB.");
    expect(result.proposal?.edl).toEqual([]);
    expect(result.proposal?.projectActions).toEqual([action]);
    expect(result.proposal?.renderReview).toBeNull();
    expect(result.preparedProposal).toEqual(preparedProposal);
    expect(result.proposalValidationIssues).toEqual([]);
  });

  it("returns review-level risk and withholds the prepared bundle for invalid proposals", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const request = {
      prompt: "Clear the timeline.",
      focus: { mediaIds: [], timelineItemIds: [] },
      createdAt: "2026-07-25T00:00:00Z",
    } satisfies CodexConversationEditRequest;
    const removal = { type: "removeItems", itemIds: ["video-1"] } as const satisfies ProjectAction;
    const reviewBundle = {
      actions: [removal],
      actionIds: ["codex-action-1-aabbccddeeff"],
      risk: {
        level: "review",
        reasons: [
          { code: "deletesExistingItems", message: "Removes existing clips from the timeline." },
        ],
      },
      impact: {
        summary: "Removes 1 item; the timeline goes from 10.0s to 0.0s.",
        beforeDurationSeconds: 10,
        afterDurationSeconds: 0,
        affectedItemIds: ["video-1"],
        affectedRanges: [{ startSeconds: 0, endSeconds: 10 }],
        previewTimestamp: 0,
      },
    } satisfies CodexPreparedProposal;
    const base = {
      project,
      threadId: "thread-1",
      threadResponse: {},
      turnResponse: {},
      proposal: { summary: "Cleared it.", edl: [], projectActions: [removal], renderReview: null },
    };

    vi.mocked(invoke).mockResolvedValueOnce({
      ...base,
      preparedProposal: reviewBundle,
      proposalValidationIssues: [],
    });
    const review = await startCodexConversationEditForProject({ project, request });
    expect(review.preparedProposal?.risk.level).toBe("review");
    expect(review.preparedProposal?.risk.reasons[0]?.code).toBe("deletesExistingItems");

    const issue = { path: "projectActions", message: "item missing", fix: "Use canonical targets" };
    vi.mocked(invoke).mockResolvedValueOnce({
      ...base,
      preparedProposal: null,
      proposalValidationIssues: [issue],
    });
    const invalid = await startCodexConversationEditForProject({ project, request });
    expect(invalid.preparedProposal).toBeNull();
    expect(invalid.proposalValidationIssues).toEqual([issue]);
  });

  it("applies a prepared conversation proposal with its action IDs and review approval", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateAudioVolume",
      itemId: "audio-1",
      volumeDb: -2,
    } as const satisfies ProjectAction;
    const proposal = {
      summary: "Lowered the interview clip by 2 dB.",
      edl: [],
      projectActions: [action],
      renderReview: null,
    } satisfies CodexConversationEditProposal;
    const applied = {
      project,
      report: { manifestPath: "/tmp/project/video-creater.project.json", writtenFiles: [], removedFiles: [] },
      historyEntryId: "agent-edit-r2",
      actionIds: ["codex-action-1-0a1b2c3d4e5f"],
      risk: { level: "safe", reasons: [] },
      impact: {
        summary: "Changes 1 item; the timeline stays 10.0s.",
        beforeDurationSeconds: 10,
        afterDurationSeconds: 10,
        affectedItemIds: ["audio-1"],
        affectedRanges: [{ startSeconds: 0, endSeconds: 10 }],
        previewTimestamp: 0,
      },
      warnings: [],
    } satisfies ProjectAgentApplyResult;
    vi.mocked(invoke).mockResolvedValue(applied);

    const result = await applyCodexConversationProposal({
      projectDir: "/tmp/project",
      proposal,
      actionIds: ["codex-action-1-0a1b2c3d4e5f"],
      reviewApproved: false,
      sessionId: "agent-session-1",
    });

    expect(invoke).toHaveBeenCalledWith("apply_codex_conversation_proposal", {
      projectDir: "/tmp/project",
      proposal,
      actionIds: ["codex-action-1-0a1b2c3d4e5f"],
      reviewApproved: false,
      sessionId: "agent-session-1",
    });
    expect(result.historyEntryId).toBe("agent-edit-r2");
    expect(result.actionIds).toEqual(["codex-action-1-0a1b2c3d4e5f"]);
    expect(result.warnings).toEqual([]);
  });

  it("undoes the latest conversation edit or reports why Undo is unavailable", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const undone = {
      status: "undone",
      project,
      report: { manifestPath: "/tmp/project/video-creater.project.json", writtenFiles: [], removedFiles: [] },
      entryId: "agent-edit-r2",
      actionCount: 1,
      remainingAgentHistory: 0,
      warnings: [],
      removedGeneratedAssetIds: [],
    } satisfies ProjectAgentUndoOutcome;
    vi.mocked(invoke).mockResolvedValueOnce(undone);

    const result = await undoLatestCodexConversationEdit({
      projectDir: "/tmp/project",
      historyEntryId: "agent-edit-r2",
    });

    expect(invoke).toHaveBeenCalledWith("undo_latest_codex_conversation_edit", {
      projectDir: "/tmp/project",
      historyEntryId: "agent-edit-r2",
    });
    expect(result).toEqual(undone);

    const conflict = {
      status: "conflict",
      entryId: "agent-edit-r2",
      message:
        "The project changed after this edit was applied, so undoing it would discard later changes.",
    } satisfies ProjectAgentUndoOutcome;
    vi.mocked(invoke).mockResolvedValueOnce(conflict);
    const refused = await undoLatestCodexConversationEdit({ projectDir: "/tmp/project" });
    expect(refused.status).toBe("conflict");
    if (refused.status !== "conflict") throw new Error("expected a conflict outcome");
    expect(refused.message).toContain("project changed");
  });

  it("loads legacy edit-job and conversation requests from app-server history", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const legacyEntry = {
      id: "app-server-turn-1",
      projectId: "project-1",
      threadId: "thread-1",
      prompt: editRequest.prompt,
      request: editRequest,
      hasProposal: true,
      threadResponse: {},
      turnResponse: {},
    } satisfies AppServerConversationEntry;
    const conversationEntry = {
      id: "app-server-turn-2",
      projectId: "project-1",
      threadId: "thread-1",
      prompt: "Remove dead air",
      request: {
        prompt: "Remove dead air",
        focus: { mediaIds: [], timelineItemIds: [] },
        createdAt: "2026-07-25T00:00:00Z",
      },
      hasProposal: false,
      threadResponse: {},
      turnResponse: {},
    } satisfies AppServerConversationEntry;
    vi.mocked(invoke).mockResolvedValue({
      schemaVersion: 1,
      entries: [legacyEntry, conversationEntry],
    });

    const history = await loadAppServerConversationHistoryFromSplitProjectFolder({
      projectDir: "/tmp/project",
    });

    expect(invoke).toHaveBeenCalledWith("load_app_server_conversation_history_from_split_project_folder", {
      projectDir: "/tmp/project",
    });
    expect(history.entries.map((entry) => entry.prompt)).toEqual([
      editRequest.prompt,
      "Remove dead air",
    ]);
  });

  it("cancels the active Codex turn for the project scope", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue(true);
    await expect(cancelCodexVideoEditForProject({ projectDir: "/tmp/project" })).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith("cancel_codex_video_edit_for_project", { projectDir: "/tmp/project" });
  });

  it("stops the in-flight conversation turn and reports whether one was running", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValueOnce(true).mockResolvedValueOnce(false);

    await expect(cancelCodexConversationEditForProject({ projectDir: "/tmp/project" })).resolves.toBe(true);
    await expect(
      cancelCodexConversationEditForProject({ projectRoot: "/tmp/root", projectDir: "/tmp/project" }),
    ).resolves.toBe(false);

    expect(invoke).toHaveBeenNthCalledWith(1, "cancel_codex_conversation_edit_for_project", {
      projectDir: "/tmp/project",
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "cancel_codex_conversation_edit_for_project", {
      projectRoot: "/tmp/root",
      projectDir: "/tmp/project",
    });
  });

  it("captures a canonical preview frame from the saved split project", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const captured = {
      project,
      playheadSeconds: 4.5,
      previewFrame: "renders/frame-job-1/preview-qa/preview-frames/preview-0001.png",
      sourceOutput: "renders/frame-job-1/output.mov",
      evidenceReport: "renders/frame-job-1/preview-qa/canonical-preview-frame.json",
    } satisfies Partial<PreparedPreviewFrameResult>;
    vi.mocked(invoke).mockResolvedValue(captured);

    const result = await captureCanonicalPreviewFrameInSplitProjectFolder({
      projectDir: "/tmp/project",
      playheadSeconds: 4.5,
      jobId: "frame-job-1",
      updatedAt: "2026-09-15T00:00:00Z",
    });

    expect(invoke).toHaveBeenCalledWith("capture_canonical_preview_frame_in_split_project_folder", {
      projectDir: "/tmp/project",
      playheadSeconds: 4.5,
      jobId: "frame-job-1",
      updatedAt: "2026-09-15T00:00:00Z",
    });
    expect(result.previewFrame).toBe(captured.previewFrame);
    expect(result.playheadSeconds).toBe(4.5);
  });

  it("accepts resize and caption edit project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const resizeAction = {
      type: "resizeItems",
      resizes: [{ itemId: "item-1", durationSeconds: 2.5 }],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    const captionAction = {
      type: "editCaptionText",
      itemId: "caption-1",
      text: "Corrected caption",
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action: resizeAction });
    await applyProjectActionToProject({ project, action: captionAction });

    expect(invoke).toHaveBeenNthCalledWith(
      1,
      "apply_project_action_to_project",
      {
        project,
        action: resizeAction,
      },
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "apply_project_action_to_project",
      {
        project,
        action: captionAction,
      },
    );
  });

  it("accepts text item edit project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "editTextItem",
      itemId: "overlay-1",
      text: "Updated overlay",
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts text overlay update project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateTextOverlayItems",
      updates: [
        {
          itemId: "overlay-1",
          startSeconds: 2.25,
          durationSeconds: 4.5,
          text: "Launch title",
          visualTreatment: "bold upper-left title with transparent backing",
          motion: "fade in quickly, hold, then drift upward",
          safeZone: "keep inside title safe margins",
          avoid: "covering faces or using opaque full-width slabs",
        },
      ],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts split project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "splitItems",
      splits: [
        {
          itemId: "item-1",
          newItemId: "item-1-b",
          splitSeconds: 1.75,
        },
      ],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts trim project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "trimItems",
      trims: [
        {
          itemId: "item-1",
          startSeconds: 0.5,
          durationSeconds: 2.5,
          sourceIn: 1,
          sourceOut: 3.5,
        },
      ],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts template update project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateTemplateItems",
      updates: [
        {
          itemId: "template-1",
          startSeconds: 4.5,
          durationSeconds: 1.25,
          templateFields: {
            headline: "Olha API",
            subline: "Founder",
          },
        },
      ],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts transcript word edit project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "editTranscriptWords",
      edits: [
        {
          transcriptId: "transcript-media-1",
          wordIndex: 1,
          text: "Creator",
          startSeconds: 0.55,
          endSeconds: 0.95,
          repairId: "repair-1",
          createdAt: "2026-06-22T10:00:00Z",
        },
      ],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts caption repair project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "applyCaptionRepair",
      repair: {
        captionItemId: "caption-1",
        transcriptId: "transcript-media-1",
        wordIndex: 1,
        text: "Creator",
        startSeconds: 0.55,
        endSeconds: 0.95,
        repairId: "repair-caption-1",
        createdAt: "2026-06-22T10:00:00Z",
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts template override project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "updateTemplateOverride",
      override: {
        templateId: "kinetic-lower-third-v1",
        name: "Kinetic Lower Third",
        fields: {
          headline: "Launch day",
          subline: "Built with Video Creater",
        },
        style: {
          accentColor: "#22d3ee",
          backgroundColor: "rgba(2, 6, 23, 0.72)",
          textColor: "#ffffff",
        },
        visualTreatment:
          "compact lower-third block with translucent backing and strong hierarchy",
        motion: "slide-and-fade in over 8 frames, hold, then soft fade out",
        safeZone: "keep essential text inside 10% margins",
        avoid: "full-width opaque black slabs and default-font template looks",
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts generated asset project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "recordGeneratedAsset",
      asset: {
        id: "generated-shot-1",
        kind: "generated",
        status: "completed",
        prompt: "slow push-in on the product",
        model: {
          provider: "seedance",
          id: "seedance-2-fast",
        },
        references: {
          mediaIds: ["media-1"],
          firstFrameMediaId: "media-1",
          lastFrameMediaId: null,
        },
        settings: {
          width: 1280,
          height: 720,
          durationSeconds: 4,
          fps: 24,
          aspectRatio: "16:9",
        },
        outputs: [
          {
            mediaId: "generated-shot-1-output",
            relativePath: "generated/generated-shot-1/output.mp4",
            width: 1280,
            height: 720,
            durationSeconds: 4,
            fps: 24,
          },
        ],
        createdAt: "2026-06-22T10:00:00Z",
        parentAssetId: null,
        retryOfAssetId: null,
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts complete generated asset project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "completeGeneratedAsset",
      assetId: "generated-shot-1",
      outputs: [
        {
          mediaId: "generated-shot-1-output",
          relativePath: "generated/generated-shot-1/output.mp4",
          sourceUrl: "https://fal.media/generated/output.mp4",
          width: 1280,
          height: 720,
          durationSeconds: 4,
          fps: 24,
        },
      ],
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts complete generated asset replacement project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "completeGeneratedAsset",
      assetId: "generated-shot-1",
      outputs: [
        {
          mediaId: "generated-shot-1-output",
          relativePath: "generated/generated-shot-1/output.mp4",
          width: 1280,
          height: 720,
          durationSeconds: 4,
          fps: 24,
        },
      ],
      replacement: {
        itemId: "generated-clip-1",
        mediaId: "generated-shot-1-output",
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts generated output replacement project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "replaceTimelineItemWithGeneratedOutput",
      replacement: {
        itemId: "generated-clip-1",
        mediaId: "generated-shot-1-output",
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });

  it("accepts render report project actions in the frontend contract", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    const action = {
      type: "attachRenderReport",
      report: {
        schemaVersion: 1,
        id: "render-draft-1",
        status: "completed",
        outputPath: "renders/render-draft-1/output.mp4",
        durationSeconds: 42.5,
        streams: {
          video: true,
          audio: true,
        },
        checks: {
          duration: "passed",
          streams: "passed",
          captionAlignment: "passed",
          overlayTiming: "passed",
          artifactPaths: "passed",
          logPath: "passed",
        },
        artifacts: [
          "renders/render-draft-1/output.mp4",
          "visual-qa/preview-001.png",
          "renders/render-draft-1/frames/frame-000030.png",
          "visual-qa/diff-001.png",
        ],
        previewComparison: {
          status: "failed",
          comparedFrames: [
            {
              timelineSeconds: 1.25,
              previewFrame: "visual-qa/preview-001.png",
              renderedFrame: "renders/render-draft-1/frames/frame-000030.png",
              diffFrame: "visual-qa/diff-001.png",
              mismatchRatio: 0.08,
              passed: false,
            },
          ],
        },
        logPath: "logs/render-draft-1.log",
        createdAt: "2026-06-22T10:00:00Z",
      },
    } as const satisfies Parameters<
      typeof applyProjectActionToProject
    >[0]["action"];
    vi.mocked(invoke).mockResolvedValue(project);

    await applyProjectActionToProject({ project, action });

    expect(invoke).toHaveBeenCalledWith("apply_project_action_to_project", {
      project,
      action,
    });
  });
});

describe("effect parameter keyframe actions", () => {
  it("derives legacy ids and applies collision-safe upsert, move, delete, and snapshot undo", () => {
    const effects = canonicalizeProjectActionEffects([
      { effectType: "stylize.grain", enabled: true, params: { amount: 0.2 } },
      { effectType: "stylize.grain", enabled: true, params: { amount: 0.4 } },
    ]);
    expect(effects.map((effect) => effect.effectInstanceId)).toEqual([
      "legacy:stylize.grain:1",
      "legacy:stylize.grain:2",
    ]);
    const base: VideoProject = {
      ...project,
      timeline: {
        durationSeconds: 4,
        tracks: [{
          id: "video",
          name: "Video",
          kind: "video",
          enabled: true,
          locked: false,
          items: [{
            id: "clip",
            kind: "video_clip",
            startSeconds: 0,
            durationSeconds: 4,
            label: "Clip",
            source: { type: "media", mediaId: "media" },
            properties: { effects: effects.map(({ effectInstanceId: _, ...effect }) => effect) },
          }],
        }],
      },
    };
    const before = structuredClone(base);
    const first = applyProjectActionLocally(base, {
      type: "upsertEffectParameterKeyframe",
      itemId: "clip",
      effectInstanceId: "legacy:stylize.grain:2",
      parameterKey: "amount",
      keyframe: { atSeconds: 1, value: 0.6, easing: "smooth" },
    });
    const moved = applyProjectActionLocally(first, {
      type: "moveEffectParameterKeyframe",
      itemId: "clip",
      effectInstanceId: "legacy:stylize.grain:2",
      parameterKey: "amount",
      fromSeconds: 1,
      toSeconds: 2,
    });
    expect(requiredTimelineItem(moved.timeline, "moved-keyframe timeline").properties.effectParameterKeyframes)
      .toEqual({
        "legacy:stylize.grain:2": {
          amount: [{ atSeconds: 2, value: 0.6, easing: "easeInOut" }],
        },
      });
    const deleted = applyProjectActionLocally(moved, {
      type: "deleteEffectParameterKeyframe",
      itemId: "clip",
      effectInstanceId: "legacy:stylize.grain:2",
      parameterKey: "amount",
      atSeconds: 2,
    });
    expect(requiredTimelineItem(deleted.timeline, "deleted-keyframe timeline").properties.effectParameterKeyframes)
      .toBeUndefined();
    // Agent undo restores the serialized before snapshot; ensure legacy input is untouched.
    expect(before).toEqual(base);
  });
});

describe("project job ordering", () => {
  it("orders recent project jobs by updated timestamp with invalid dates last", () => {
    const jobs: ProjectJobSummary[] = [
      {
        id: "older",
        kind: "render_draft",
        status: "queued",
        updatedAt: "2026-06-23T12:00:00Z",
      },
      {
        id: "invalid",
        kind: "generate_media",
        status: "running",
        updatedAt: "not-a-date",
      },
      {
        id: "newest",
        kind: "generate_media",
        status: "running",
        updatedAt: "2026-06-23T12:05:00Z",
      },
      {
        id: "same-a",
        kind: "codex_edit",
        status: "progress",
        updatedAt: "2026-06-23T12:02:00Z",
      },
      {
        id: "same-b",
        kind: "transcribe_media",
        status: "blocked",
        updatedAt: "2026-06-23T12:02:00Z",
      },
    ];

    expect(orderRecentProjectJobs(jobs, 4).map((job) => job.id)).toEqual([
      "newest",
      "same-a",
      "same-b",
      "older",
    ]);
  });
});

describe("local job failure actions (Rust project_action/job_failure.rs)", () => {
  function projectWithJob(status: ProjectJobSummary["status"]): VideoProject {
    return {
      ...project,
      jobs: [
        {
          id: "job-stale",
          kind: "export_media",
          status,
          updatedAt: "2026-09-16T10:00:00Z",
        },
      ],
    };
  }
  const recordFailure = {
    type: "recordJobFailure",
    jobId: "job-stale",
    reason: "The workflow never started.",
    updatedAt: "2026-09-16T10:05:00Z",
    runId: null,
  } as const satisfies ProjectAction;

  it("fails a queued job with its reason", () => {
    const next = applyProjectActionLocally(projectWithJob("queued"), recordFailure);

    expect(next.jobs[0]).toEqual({
      id: "job-stale",
      kind: "export_media",
      status: "failed",
      updatedAt: "2026-09-16T10:05:00Z",
      failureReason: "The workflow never started.",
    });
  });

  it("rejects the fields and missing jobs Rust rejects", () => {
    const queued = projectWithJob("queued");
    const cases: [Partial<Extract<ProjectAction, { type: "recordJobFailure" }>>, string][] = [
      [{ jobId: " " }, "job id cannot be empty"],
      [{ jobId: "../job-stale" }, "job id must be a safe path segment: ../job-stale"],
      [{ jobId: "." }, "job id must be a safe path segment: ."],
      [{ reason: "  " }, "job failure reason cannot be empty"],
      [{ updatedAt: "" }, "job updatedAt cannot be empty"],
      [{ runId: " " }, "job workflow metadata is missing: runId"],
      [{ jobId: "job-missing" }, "job was not found: job-missing"],
    ];
    for (const [overrides, message] of cases) {
      expect(() => applyProjectActionLocally(queued, { ...recordFailure, ...overrides })).toThrow(message);
    }
  });

  it("leaves a completed job unchanged", () => {
    const completed = projectWithJob("completed");

    expect(applyProjectActionLocally(completed, recordFailure)).toBe(completed);
  });

  it("drops the failure reason when the job status moves back to running", () => {
    const failed = applyProjectActionLocally(projectWithJob("queued"), recordFailure);
    const retried = applyProjectActionLocally(failed, {
      type: "updateJobStatus",
      jobId: "job-stale",
      status: "running",
      updatedAt: "2026-09-16T10:06:00Z",
    });

    expect(retried.jobs[0]?.status).toBe("running");
    expect(retried.jobs[0]).not.toHaveProperty("failureReason");
  });
});

describe("local transcript repair actions", () => {
  // Mirrors the Rust `transcript_with_words` / `caption_repair_action` fixtures in
  // src-tauri/tests/project_action.rs.
  function transcriptProject(): VideoProject {
    return {
      ...project,
      media: [
        {
          id: "media-1",
          relativePath: "media/input.mp4",
          kind: "video",
          durationSeconds: 4,
          width: 1920,
          height: 1080,
          fps: 24,
        },
      ],
      transcripts: [
        {
          id: "transcript-media-1",
          mediaId: "media-1",
          engine: "nvidia/parakeet-tdt-0.6b-v3",
          rawArtifactPath: null,
          repairs: [],
          segments: [],
          words: [
            { text: "Video", startSeconds: 0, endSeconds: 0.4, confidence: 0.95, speaker: null },
            { text: "Creater", startSeconds: 0.5, endSeconds: 0.9, confidence: 0.88, speaker: "host" },
          ],
        },
      ],
      timeline: {
        durationSeconds: 3,
        tracks: [
          {
            id: "track-video",
            name: "Video",
            kind: "video",
            locked: false,
            items: [
              {
                id: "item-1",
                kind: "video_clip",
                startSeconds: 0,
                durationSeconds: 3,
                source: { type: "media", mediaId: "media-1" },
                label: "Clip",
                properties: {},
              },
            ],
          },
          {
            id: "track-captions",
            name: "Captions",
            kind: "caption",
            locked: false,
            items: [
              {
                id: "caption-1",
                kind: "caption",
                startSeconds: 0.5,
                durationSeconds: 1,
                source: { type: "text", text: "Caption" },
                label: "Caption",
                properties: { textEdited: false },
              },
              {
                id: "caption-2",
                kind: "caption",
                startSeconds: 1.2,
                durationSeconds: 0.2,
                source: { type: "text", text: "Later" },
                label: "Later",
                properties: {},
              },
            ],
          },
        ],
      },
    };
  }

  function wordEdit(
    edit: Partial<Extract<ProjectAction, { type: "editTranscriptWords" }>["edits"][number]> = {},
  ): ProjectAction {
    return {
      type: "editTranscriptWords",
      edits: [
        {
          transcriptId: "transcript-media-1",
          wordIndex: 1,
          text: "Creator",
          repairId: "repair-1",
          createdAt: "2026-06-22T10:00:00Z",
          ...edit,
        },
      ],
    };
  }

  function captionRepair(
    repair: Partial<Extract<ProjectAction, { type: "applyCaptionRepair" }>["repair"]> = {},
  ): ProjectAction {
    return {
      type: "applyCaptionRepair",
      repair: {
        captionItemId: "caption-1",
        transcriptId: "transcript-media-1",
        wordIndex: 1,
        text: "Creator",
        startSeconds: 1.5,
        endSeconds: 1.9,
        repairId: "repair-caption-1",
        createdAt: "2026-06-22T10:00:00Z",
        ...repair,
      },
    };
  }

  it("edits transcript words and records repair metadata like Rust EditTranscriptWords", () => {
    const base = transcriptProject();
    const next = applyProjectActionLocally(
      base,
      wordEdit({ text: "  Creator  ", startSeconds: 0.55, endSeconds: 0.95 }),
    );

    const transcript = requiredAt(next.transcripts, 0, "transcript");
    expect(transcript.words[1]).toEqual({
      text: "Creator",
      startSeconds: 0.55,
      endSeconds: 0.95,
      confidence: 0.88,
      speaker: "host",
    });
    expect(transcript.repairs).toEqual([
      {
        id: "repair-1",
        kind: "word_text_and_timing",
        wordIndex: 1,
        before: { text: "Creater", startSeconds: 0.5, endSeconds: 0.9, confidence: 0.88, speaker: "host" },
        after: { text: "Creator", startSeconds: 0.55, endSeconds: 0.95, confidence: 0.88, speaker: "host" },
        createdAt: "2026-06-22T10:00:00Z",
      },
    ]);
    expect(next.timeline).toBe(base.timeline);
    expect(base.transcripts[0]?.words[1]?.text).toBe("Creater");
    expect(base.transcripts[0]?.repairs).toEqual([]);
  });

  it("applies every transcript word edit in order and classifies each repair", () => {
    const next = applyProjectActionLocally(transcriptProject(), {
      type: "editTranscriptWords",
      edits: [
        { transcriptId: "transcript-media-1", wordIndex: 1, text: "Creator", repairId: "r-text", createdAt: "t1" },
        { transcriptId: "transcript-media-1", wordIndex: 0, endSeconds: 0.45, repairId: "r-timing", createdAt: "t2" },
        { transcriptId: "transcript-media-1", wordIndex: 0, repairId: "r-noop", createdAt: "t3" },
      ],
    });

    const transcript = requiredAt(next.transcripts, 0, "transcript");
    expect(transcript.words.map((word) => [word.text, word.startSeconds, word.endSeconds])).toEqual([
      ["Video", 0, 0.45],
      ["Creator", 0.5, 0.9],
    ]);
    expect(transcript.repairs.map((repair) => [repair.id, repair.kind, repair.wordIndex, repair.createdAt])).toEqual([
      ["r-text", "word_text", 1, "t1"],
      ["r-timing", "word_timing", 0, "t2"],
      ["r-noop", "word_timing", 0, "t3"],
    ]);
  });

  it("rejects invalid transcript word edits without changing the project", () => {
    const base = transcriptProject();
    const rejected: ProjectAction[] = [
      { type: "editTranscriptWords", edits: [] },
      wordEdit({ transcriptId: "missing-transcript" }),
      wordEdit({ wordIndex: 99 }),
      wordEdit({ wordIndex: -1 }),
      wordEdit({ text: "   " }),
      wordEdit({ startSeconds: 0.2, endSeconds: 0.95 }),
      wordEdit({ startSeconds: -0.1 }),
      wordEdit({ startSeconds: 0.8, endSeconds: 0.7 }),
      wordEdit({ endSeconds: 4.5 }),
      wordEdit({ endSeconds: Number.NaN }),
      {
        type: "editTranscriptWords",
        edits: [
          { transcriptId: "transcript-media-1", wordIndex: 1, text: "Creator", repairId: "ok", createdAt: "t" },
          { transcriptId: "transcript-media-1", wordIndex: 7, text: "Nope", repairId: "bad", createdAt: "t" },
        ],
      },
    ];
    for (const action of rejected) {
      expect(applyProjectActionLocally(base, action)).toBe(base);
    }
    const withoutMedia = { ...base, media: [] };
    expect(applyProjectActionLocally(withoutMedia, wordEdit())).toBe(withoutMedia);
  });

  it("applies a caption repair to the caption and transcript like Rust ApplyCaptionRepair", () => {
    const base = transcriptProject();
    const next = applyProjectActionLocally(base, captionRepair());

    const captions = requiredAt(next.timeline.tracks, 1, "caption track").items;
    expect(captions.map((item) => item.id)).toEqual(["caption-2", "caption-1"]);
    const caption = requiredAt(captions, 1, "repaired caption");
    expect(caption.startSeconds).toBe(1.5);
    expect(caption.durationSeconds).toBeCloseTo(0.4, 10);
    expect(caption.source).toEqual({ type: "text", text: "Creator" });
    expect(caption.label).toBe("Caption");
    expect(caption.properties).toEqual({
      textEdited: true,
      captionRepairId: "repair-caption-1",
      transcriptId: "transcript-media-1",
      wordIndex: 1,
    });
    expect(next.timeline.durationSeconds).toBe(3);
    expect(next.timelines?.[0]?.timeline).toBe(next.timeline);

    const transcript = requiredAt(next.transcripts, 0, "transcript");
    expect(transcript.words[1]).toEqual({
      text: "Creator",
      startSeconds: 1.5,
      endSeconds: 1.9,
      confidence: 0.88,
      speaker: "host",
    });
    expect(transcript.repairs).toEqual([
      {
        id: "repair-caption-1",
        kind: "word_text_and_timing",
        wordIndex: 1,
        before: { text: "Creater", startSeconds: 0.5, endSeconds: 0.9, confidence: 0.88, speaker: "host" },
        after: { text: "Creator", startSeconds: 1.5, endSeconds: 1.9, confidence: 0.88, speaker: "host" },
        createdAt: "2026-06-22T10:00:00Z",
      },
    ]);
    expect(base.timeline.tracks[1]?.items[0]?.source).toEqual({ type: "text", text: "Caption" });
  });

  it("trims caption repair text and keeps the timing when only the text changes", () => {
    const next = applyProjectActionLocally(
      transcriptProject(),
      captionRepair({ text: "  Creator  ", startSeconds: 0.5, endSeconds: 0.9 }),
    );
    const caption = next.timeline.tracks[1]?.items.find((item) => item.id === "caption-1");
    expect(caption?.source).toEqual({ type: "text", text: "Creator" });
    expect(next.transcripts[0]?.repairs[0]?.kind).toBe("word_text");
  });

  it("rejects invalid caption repairs without changing the project", () => {
    const base = transcriptProject();
    const lockedCaptions = {
      ...base,
      timeline: {
        ...base.timeline,
        tracks: base.timeline.tracks.map((track) =>
          track.kind === "caption" ? { ...track, locked: true } : track,
        ),
      },
    };
    const mediaCaption = {
      ...base,
      timeline: {
        ...base.timeline,
        tracks: base.timeline.tracks.map((track) => ({
          ...track,
          items: track.items.map((item) =>
            item.id === "caption-1"
              ? { ...item, source: { type: "media" as const, mediaId: "media-1" } }
              : item,
          ),
        })),
      },
    };
    const cases: [VideoProject, ProjectAction][] = [
      [base, captionRepair({ text: "  " })],
      [base, captionRepair({ startSeconds: -0.1 })],
      [base, captionRepair({ startSeconds: Number.POSITIVE_INFINITY })],
      [base, captionRepair({ endSeconds: 1.5 })],
      [base, captionRepair({ captionItemId: "missing" })],
      [lockedCaptions, captionRepair()],
      [base, captionRepair({ captionItemId: "item-1" })],
      [mediaCaption, captionRepair()],
      [base, captionRepair({ transcriptId: "missing-transcript" })],
      [{ ...base, media: [] }, captionRepair()],
      [base, captionRepair({ wordIndex: 2 })],
      [base, captionRepair({ wordIndex: 0.5 })],
      [base, captionRepair({ endSeconds: 4.5 })],
      [base, captionRepair({ wordIndex: 0, startSeconds: 0, endSeconds: 0.6 })],
    ];
    for (const [input, action] of cases) {
      expect(applyProjectActionLocally(input, action)).toBe(input);
    }
  });
});

describe("transition actions and maintenance (Rust project_action transition tests)", () => {
  const apply = (project: VideoProject, action: ProjectAction) => applyProjectActionLocally(project, action);
  const transitionsOf = (project: VideoProject) => trackOf(project, VIDEO_TRACK).transitions ?? [];
  const fade = (left = "clip-a", right = "clip-b", duration = 1) => transition("fade-1", left, right, duration);

  it("matches the Rust camelCase wire shapes", () => {
    const actions: ProjectAction[] = [
      { type: "addTransition", trackId: VIDEO_TRACK, transition: fade() },
      { type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", kind: "wipe", durationSeconds: 0.5 },
      { type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", kind: "dipToWhite" },
      { type: "removeTransition", trackId: VIDEO_TRACK, transitionId: "fade-1" },
    ];
    expect(JSON.parse(JSON.stringify(actions))).toEqual([
      {
        type: "addTransition",
        trackId: VIDEO_TRACK,
        transition: { id: "fade-1", leftItemId: "clip-a", rightItemId: "clip-b", kind: "crossfade", durationSeconds: 1 },
      },
      { type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", kind: "wipe", durationSeconds: 0.5 },
      { type: "updateTransition", trackId: VIDEO_TRACK, transitionId: "fade-1", kind: "dipToWhite" },
      { type: "removeTransition", trackId: VIDEO_TRACK, transitionId: "fade-1" },
    ]);
    expect(cutProject().timeline.tracks[0]).not.toHaveProperty("transitions");
  });

  it("drops the transition when the right clip moves away, in the library entry too", () => {
    const project = apply(projectWithCrossfade(), {
      type: "moveItems",
      moves: [{ itemId: "clip-b", targetTrackId: VIDEO_TRACK, startSeconds: 6 }],
    });
    expect(transitionsOf(project)).toEqual([]);
    expect(project.timelines?.find((entry) => entry.id === "main")?.timeline).toBe(project.timeline);
    expect(project.timeline.tracks[0]).not.toHaveProperty("transitions");
  });

  it("keeps the transition when both clips move together", () => {
    const project = apply(projectWithCrossfade(), {
      type: "moveItems",
      moves: [
        { itemId: "clip-b", targetTrackId: VIDEO_TRACK, startSeconds: 14 },
        { itemId: "clip-a", targetTrackId: VIDEO_TRACK, startSeconds: 10 },
      ],
    });
    expect(transitionsOf(project)).toEqual([fade()]);
  });

  it("drops the transition when a clip moves to another track", () => {
    const base = projectWithCrossfade();
    const second: TimelineTrack = { id: "track-video-2", name: "Video 2", kind: "video", locked: false, enabled: true, items: [] };
    const withTrack = apply(base, { type: "createTrack", track: second });
    const project = apply(withTrack, {
      type: "moveItems",
      moves: [{ itemId: "clip-b", targetTrackId: "track-video-2", startSeconds: 4 }],
    });
    expect(transitionsOf(project)).toEqual([]);
    expect(trackOf(project, "track-video-2").transitions).toBeUndefined();
  });

  it("clamps the duration when trimming the left clip's sourceOut", () => {
    const project = apply(projectWithCrossfade(2), {
      type: "trimItems",
      trims: [{ itemId: "clip-a", startSeconds: 0, durationSeconds: 4, sourceIn: 7.75, sourceOut: 11.75 }],
    });
    expect(transitionsOf(project)).toHaveLength(1);
    expect(transitionsOf(project)[0]?.durationSeconds).toBeCloseTo(0.5, 6);
  });

  it("keeps adjacency and clamps when ripple trimming the left clip", () => {
    const project = apply(projectWithCrossfade(2), {
      type: "rippleTrimItem",
      itemId: "clip-a",
      edge: "right",
      deltaSeconds: 5.5,
      propagateLinked: false,
      syncLockedTrackIds: [],
    });
    // clip-a now ends at source 11.5 of 12: 0.5 s of tail handle allows 1.0 s.
    expect(trackOf(project, VIDEO_TRACK).items[1]?.startSeconds).toBeCloseTo(9.5, 6);
    expect(transitionsOf(project)).toHaveLength(1);
    expect(transitionsOf(project)[0]?.durationSeconds).toBeCloseTo(1, 6);
  });

  it("drops the transition when resizing the left clip away from the cut", () => {
    const project = apply(projectWithCrossfade(), {
      type: "resizeItems",
      resizes: [{ itemId: "clip-a", durationSeconds: 3 }],
    });
    expect(transitionsOf(project)).toEqual([]);
  });

  it("re-targets to the right-hand piece when splitting the left clip", () => {
    const split = (seconds: number) =>
      apply(projectWithCrossfade(), {
        type: "splitItems",
        splits: [{ itemId: "clip-a", newItemId: "clip-a-tail", splitSeconds: seconds }],
      });
    expect(transitionsOf(split(2))).toEqual([fade("clip-a-tail")]);

    const nearCut = transitionsOf(split(3.75));
    expect(nearCut).toHaveLength(1);
    expect(nearCut[0]?.leftItemId).toBe("clip-a-tail");
    expect(nearCut[0]?.durationSeconds).toBeCloseTo(0.25, 6);
  });

  it("keeps the transition and clamps to the head when splitting the right clip", () => {
    const project = apply(projectWithCrossfade(2), {
      type: "splitItems",
      splits: [{ itemId: "clip-b", newItemId: "clip-b-tail", splitSeconds: 5.5 }],
    });
    expect(transitionsOf(project)).toHaveLength(1);
    expect(transitionsOf(project)[0]?.rightItemId).toBe("clip-b");
    expect(transitionsOf(project)[0]?.durationSeconds).toBeCloseTo(1.5, 6);
  });

  it("drops the transition when either clip is removed", () => {
    for (const removed of ["clip-a", "clip-b"]) {
      expect(transitionsOf(apply(projectWithCrossfade(), { type: "removeItems", itemIds: [removed] }))).toEqual([]);
    }
  });

  it("keeps the transition through a ripple delete that keeps adjacency", () => {
    const project = apply(projectWithCrossfade(), {
      type: "rippleDeleteRanges",
      ranges: [{ startSeconds: 3, endSeconds: 4, trackIds: [VIDEO_TRACK] }],
    });
    expect(trackOf(project, VIDEO_TRACK).items[1]?.startSeconds).toBeCloseTo(3, 6);
    expect(transitionsOf(project)).toEqual([fade()]);
  });

  it("re-targets to the remaining tail after a ripple delete inside the left clip", () => {
    const project = apply(projectWithCrossfade(), {
      type: "rippleDeleteRanges",
      ranges: [{ startSeconds: 1, endSeconds: 2, trackIds: [VIDEO_TRACK] }],
    });
    const items = trackOf(project, VIDEO_TRACK).items;
    expect(items).toHaveLength(3);
    expect(items[1]?.id).not.toBe("clip-a");
    expect(transitionsOf(project)).toEqual([fade(items[1]?.id)]);
  });

  it("re-targets to the shifted tail when inserting into the left clip", () => {
    const project = apply(projectWithCrossfade(), {
      type: "insertItems",
      targetTrackId: VIDEO_TRACK,
      insertSeconds: 2,
      items: [clip("inserted", "Insert", 0, 1, 0)],
    });
    const tail = trackOf(project, VIDEO_TRACK).items.find(
      (item) => item.id.startsWith("clip-a-") && item.startSeconds === 3,
    );
    expect(tail).toBeDefined();
    expect(transitionsOf(project)).toEqual([fade(tail?.id)]);
  });

  it("clamps the duration to the speed-scaled handle after a speed change", () => {
    const withRange = withVideoItem(cutProject(), 0, (item) => withSourceRange(item, 7, 11));
    const added = apply(withRange, { type: "addTransition", trackId: VIDEO_TRACK, transition: fade("clip-a", "clip-b", 2) });
    expect(transitionsOf(added)).toHaveLength(1);
    const project = apply(added, { type: "updateVisualClipSpeed", itemId: "clip-a", speed: 4 });
    // 1 s of source at 4x is 0.25 s of timeline on the left side.
    expect(transitionsOf(project)).toHaveLength(1);
    expect(transitionsOf(project)[0]?.durationSeconds).toBeCloseTo(0.5, 6);
  });

  it("drops the transition when the clips' media is deleted", () => {
    expect(transitionsOf(apply(projectWithCrossfade(), { type: "deleteMedia", mediaIds: ["media-1"] }))).toEqual([]);
  });

  it("drops transitions across a one-frame gap once the frame rate rises", () => {
    const gap = withVideoItem(cutProject(), 1, (item) => ({ ...item, startSeconds: 4.04 }));
    const added = apply(gap, { type: "addTransition", trackId: VIDEO_TRACK, transition: fade() });
    expect(transitionsOf(added)).toHaveLength(1);
    // `updateRenderSettings` is backend-only, so exercise maintenance directly.
    const project = maintainTransitions({ ...added, renderSettings: { ...added.renderSettings, fps: 60 } });
    expect(transitionsOf(project)).toEqual([]);
    expect(project.timelines?.[0]?.timeline.tracks[0]?.transitions).toBeUndefined();
  });

  it("maintains library timelines on any action and keeps untouched snapshots by reference", () => {
    const base = projectWithCrossfade();
    const broken = {
      ...base.timeline,
      tracks: base.timeline.tracks.map((track) => ({
        ...track,
        transitions: [transition("stale", "clip-a", "missing", 1)],
      })),
    };
    const project: VideoProject = {
      ...base,
      timelines: [...(base.timelines ?? []), { id: "other", name: "Other", timeline: broken }],
    };
    const next = apply(project, { type: "updateVisualClipOpacity", itemId: "clip-a", opacity: 0.5 });
    expect(next.timelines?.find((entry) => entry.id === "other")?.timeline.tracks[0]).not.toHaveProperty("transitions");
    expect(maintainTransitions(next)).toBe(next);
  });

  it("decomposes a nested sequence with remapped transition ids", () => {
    const decompose = (wrapperDuration: number) => {
      const base = projectWithCrossfade();
      const timeline = {
        ...base.timeline,
        tracks: base.timeline.tracks.map((track) => {
          if (track.id !== VIDEO_TRACK) return track;
          const { transitions: _transitions, ...rest } = track;
          return {
            ...rest,
            items: [
              {
                id: "wrapper",
                kind: "video_clip" as const,
                startSeconds: 0,
                durationSeconds: wrapperDuration,
                source: { type: "timeline" as const, timelineId: "nested" },
                label: "Nested",
                properties: {},
              },
            ],
          };
        }),
      };
      const project: VideoProject = {
        ...base,
        timeline,
        timelines: [
          { id: "main", name: "Timeline 1", timeline },
          { id: "nested", name: "Nested", timeline: base.timeline },
        ],
      };
      const next = apply(project, { type: "decomposeTimelineItem", itemId: "wrapper" });
      const decomposed = next.timeline.tracks.find((track) => track.id.startsWith("decomposed-wrapper-track-video"));
      return { next, decomposed };
    };

    const { next, decomposed } = decompose(8);
    expect(decomposed?.items.map((item) => item.id)).toEqual(["wrapper-clip-a", "wrapper-clip-b"]);
    expect(decomposed?.transitions).toEqual([transition("wrapper-fade-1", "wrapper-clip-a", "wrapper-clip-b", 1)]);
    expect(next.timelines?.find((entry) => entry.id === "nested")?.timeline.tracks[0]?.transitions).toEqual([fade()]);

    // The wrapper keeps only 0.5 s of clip-b, which caps the transition.
    const clipped = decompose(4.5).decomposed;
    expect(clipped?.transitions).toHaveLength(1);
    expect(clipped?.transitions?.[0]?.durationSeconds).toBeCloseTo(0.5, 6);
  });

  it("copies transitions when duplicating a timeline", () => {
    const project = apply(projectWithCrossfade(), {
      type: "createTimeline",
      timelineId: "copy",
      name: "Copy",
      duplicateActive: true,
    });
    expect(project.activeTimelineId).toBe("copy");
    expect(transitionsOf(project)).toEqual([fade()]);
    expect(project.timelines?.find((entry) => entry.id === "main")?.timeline.tracks[0]?.transitions).toHaveLength(1);
    expect(project.timeline.tracks[0]?.items[0]?.id).toBe("clip-a");
  });

  it("leaves transitions untouched by unrelated edits", () => {
    const base = projectWithCrossfade();
    const project = apply(base, { type: "updateVisualClipOpacity", itemId: "clip-a", opacity: 0.5 });
    expect(transitionsOf(project)).toBe(transitionsOf(base));
  });

  it("maintains transitions after local timeline patches", () => {
    const project = applyTimelinePatchLocally(projectWithCrossfade(), {
      type: "moveItem",
      itemId: "clip-b",
      targetTrackId: VIDEO_TRACK,
      startSeconds: 6,
    });
    expect(transitionsOf(project)).toEqual([]);
  });
});
