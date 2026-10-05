import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, ProjectJobSummary, Transcript, VideoProject } from "@/lib/project";
import { defaultAppPreferences } from "@/lib/app-settings";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore, type EditorStore } from "../store/editor-store";
import { createSpeechService } from "./speech-service";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

function mockBackend(handlers: Record<string, Handler>) {
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = handlers[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
}

function setup(project: VideoProject = fixtureProject(), projectDir = "/projects/demo", backend: "inProcess" | "temporal" = "inProcess") {
  const store = createEditorStore({ projectDir, project });
  const applied: ProjectAction[][] = [];
  const applyActions = store.getState().applyActions;
  store.setState({
    applyActions: (actions, options) => {
      applied.push([...actions]);
      return applyActions(actions, options);
    },
  });
  const preferences = () => ({ ...defaultAppPreferences, generationExecutionBackend: backend });
  return { store, applied, service: createSpeechService(store, { preferences }) };
}

function items(store: EditorStore) {
  return store.getState().project.timeline.tracks.flatMap((track) => track.items);
}

const transcript: Transcript = {
  id: "transcript-build",
  mediaId: "media-1",
  repairs: [],
  segments: [],
  words: [
    { text: "Make", startSeconds: 0.5, endSeconds: 0.8 },
    { text: "this", startSeconds: 0.8, endSeconds: 1 },
    { text: "sing", startSeconds: 1, endSeconds: 1.4 },
  ],
};

describe("speech service", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
  });

  describe("transcribe", () => {
    it("builds the Temporal start request, records the job and starts it", async () => {
      const startRequest = { workflowId: "wf", workflowType: "VideoCreaterTranscribeMediaWorkflow", taskQueue: "q", input: {}, searchAttributes: {}, activityTypes: [], idReusePolicy: "rejectDuplicate" };
      mockBackend({
        build_temporal_job_summary: (input) => ({ id: input.jobId, kind: input.kind, status: "queued", updatedAt: input.updatedAt }),
        build_temporal_transcribe_media_start_request: () => startRequest,
        start_temporal_workflow: () => ({ status: "started", workflowId: "wf", workflowType: "t", taskQueue: "q", runId: "run-1", message: "" }),
        build_temporal_start_result_action: (input) => ({
          type: "updateJobStatus",
          jobId: (input.job as ProjectJobSummary).id,
          status: "running",
          updatedAt: input.updatedAt,
          runId: input.runId,
        }),
      });
      const { store, applied, service } = setup();

      await expect(service.transcribe("media-1")).resolves.toBe(true);

      expect(backendRequest).toHaveBeenCalledWith("build_temporal_transcribe_media_start_request", {
        projectId: "project-sample",
        projectDir: "/projects/demo",
        mediaId: "media-1",
        jobId: expect.stringMatching(/^transcribe-media-1-/),
        languageMode: "auto",
      });
      expect(applied[0]).toEqual([{ type: "recordJob", job: expect.objectContaining({ kind: "transcribe_media", startRequest }) }]);
      expect(applied[1]).toEqual([expect.objectContaining({ type: "updateJobStatus", status: "running", runId: "run-1" })]);
      const job = store.getState().project.jobs.find((entry) => entry.kind === "transcribe_media");
      expect(job).toMatchObject({ status: "running", startRequest });
    });

    it("falls back to the local start request when the backend is unavailable", async () => {
      mockBackend({});
      const { store, applied, service } = setup();

      await expect(service.transcribe("media-1")).resolves.toBe(false);

      const [recordJob] = applied[0] ?? [];
      expect(recordJob).toMatchObject({
        type: "recordJob",
        job: {
          kind: "transcribe_media",
          status: "queued",
          startRequest: { workflowType: "VideoCreaterTranscribeMediaWorkflow", input: { mediaId: "media-1", languageMode: "auto" } },
        },
      });
      expect(store.getState().lastError).toBe("A backend connection is required for this operation");
    });

    it("reports a workflow that did not start", async () => {
      mockBackend({ start_temporal_workflow: () => ({ status: "unavailable", runId: null, message: "Temporal is not running." }) });
      const { store, applied, service } = setup();
      await expect(service.transcribe("media-1")).resolves.toBe(false);
      expect(applied).toHaveLength(2);
      expect(applied[1]).toEqual([expect.objectContaining({ type: "recordJobFailure", reason: "The workflow never started.", runId: null })]);
      expect(store.getState().project.jobs.find((job) => job.kind === "transcribe_media")).toMatchObject({ status: "failed", failureReason: "The workflow never started." });
      expect(store.getState().lastError).toBe("Temporal is not running.");
    });

    it("fails the transcription job when starting its workflow throws", async () => {
      mockBackend({
        start_temporal_workflow: () => {
          throw new Error("Connection refused");
        },
      });
      const { store, service } = setup();
      await expect(service.transcribe("media-1")).resolves.toBe(false);
      expect(store.getState().project.jobs.find((job) => job.kind === "transcribe_media")?.status).toBe("failed");
      expect(store.getState().lastError).toContain("Connection refused");
    });

    const queuedJobCommands = {
      build_temporal_job_summary: (input: Record<string, unknown>) => ({ id: input.jobId, kind: input.kind, status: "queued", updatedAt: input.updatedAt }),
      build_temporal_transcribe_media_start_request: (input: Record<string, unknown>) => ({
        workflowId: "wf",
        workflowType: "VideoCreaterTranscribeMediaWorkflow",
        taskQueue: "q",
        input: { mediaId: input.mediaId, jobId: input.jobId },
        searchAttributes: {},
        activityTypes: [],
        idReusePolicy: "rejectDuplicate",
      }),
    };

    it("runs the recorded job in the backend process and loads its transcript", async () => {
      const start = vi.fn();
      const { store, applied, service } = setup();
      mockBackend({
        ...queuedJobCommands,
        start_temporal_workflow: start,
        run_transcribe_media_in_process: (input) => {
          const request = input.startRequest as { input: { jobId: string } };
          const project = store.getState().project;
          return {
            ...project,
            jobs: project.jobs.map((job) => (job.id === request.input.jobId ? { ...job, status: "completed" } : job)),
            transcripts: [transcript],
          };
        },
      });

      await expect(service.transcribe("media-1", "en")).resolves.toBe(true);

      expect(start).not.toHaveBeenCalled();
      expect(applied).toHaveLength(1);
      expect(backendRequest).toHaveBeenCalledWith("run_transcribe_media_in_process", {
        startRequest: expect.objectContaining({ workflowType: "VideoCreaterTranscribeMediaWorkflow", input: expect.objectContaining({ mediaId: "media-1" }) }),
        updatedAt: expect.any(String),
      });
      expect(store.getState().project.jobs.find((job) => job.kind === "transcribe_media")?.status).toBe("completed");
      expect(store.getState().project.transcripts).toEqual([transcript]);
      expect(store.getState().lastError).toBeNull();
    });

    it("shows the backend's failure reason when the in-process run fails", async () => {
      const { store, service } = setup();
      let failed: VideoProject | null = null;
      mockBackend({
        ...queuedJobCommands,
        run_transcribe_media_in_process: (input) => {
          const request = input.startRequest as { input: { jobId: string } };
          const project = store.getState().project;
          failed = { ...project, jobs: project.jobs.map((job) => (job.id === request.input.jobId ? { ...job, status: "failed", failureReason: "No transcription model is installed." } : job)) };
          throw new Error("No transcription model is installed.");
        },
        load_split_project_from_folder: () => failed,
      });

      await expect(service.transcribe("media-1")).resolves.toBe(false);

      expect(store.getState().project.jobs.find((job) => job.kind === "transcribe_media")).toMatchObject({ status: "failed", failureReason: "No transcription model is installed." });
      expect(store.getState().lastError).toBe("No transcription model is installed.");
    });

    it("starts a Temporal workflow instead when Temporal execution is selected", async () => {
      const run = vi.fn();
      const { store, service } = setup(fixtureProject(), "/projects/demo", "temporal");
      mockBackend({
        ...queuedJobCommands,
        run_transcribe_media_in_process: run,
        start_temporal_workflow: () => ({ status: "started", workflowId: "wf", workflowType: "t", taskQueue: "q", runId: "run-1", message: "" }),
        build_temporal_start_result_action: (input) => ({ type: "updateJobStatus", jobId: (input.job as ProjectJobSummary).id, status: "running", updatedAt: input.updatedAt, runId: input.runId }),
      });

      await expect(service.transcribe("media-1")).resolves.toBe(true);

      expect(run).not.toHaveBeenCalled();
      expect(store.getState().project.jobs.find((job) => job.kind === "transcribe_media")?.status).toBe("running");
    });

    it("blocks missing media without calling the backend", async () => {
      const { store, service } = setup();
      await expect(service.transcribe("gone")).resolves.toBe(false);
      expect(backendRequest).not.toHaveBeenCalled();
      expect(store.getState().lastError).toBe("That media is no longer in the project.");
    });
  });

  it("builds captions as one batch (track plus items) and selects them", async () => {
    const project = fixtureProject();
    project.timeline.tracks = project.timeline.tracks.filter((track) => track.kind !== "caption");
    const { store, applied, service } = setup(project, "");

    await expect(
      service.buildCaptions({ transcript, range: { startSeconds: 0, endSeconds: 2 }, wordsPerCue: 2, stylePreset: "boldReadableLower" }),
    ).resolves.toBe(true);

    expect(applied).toHaveLength(1);
    expect(applied[0]?.map((action) => action.type)).toEqual(expect.arrayContaining(["createTrack", "addItems"]));
    const captions = items(store).filter((item) => item.kind === "caption");
    expect(captions.map((item) => item.source)).toEqual([
      { type: "text", text: "Make this" },
      { type: "text", text: "sing" },
    ]);
    expect(store.getState().selectedItemIds).toEqual(captions.map((item) => item.id));
  });

  it("masks profanity in built cue text when censoring", async () => {
    const profane: Transcript = {
      ...transcript,
      words: [
        { text: "What", startSeconds: 0.5, endSeconds: 0.8 },
        { text: "the", startSeconds: 0.8, endSeconds: 1 },
        { text: "Shit!", startSeconds: 1, endSeconds: 1.4 },
      ],
    };
    const { store, service } = setup(fixtureProject(), "");

    await expect(
      service.buildCaptions({ transcript: profane, range: { startSeconds: 0, endSeconds: 2 }, wordsPerCue: 3, stylePreset: "boldReadableLower", censorProfanity: true }),
    ).resolves.toBe(true);

    const built = items(store).filter((item) => store.getState().selectedItemIds.includes(item.id));
    expect(built.map((item) => item.source)).toEqual([{ type: "text", text: "What the S***!" }]);
  });

  it("replaces existing cues in the same batch when regenerating", async () => {
    const { store, applied, service } = setup(fixtureProject(), "");

    await expect(
      service.buildCaptions({
        transcript,
        range: { startSeconds: 0, endSeconds: 2 },
        wordsPerCue: 3,
        stylePreset: "boldReadableLower",
        replaceItemIds: ["caption-1", "caption-2"],
      }),
    ).resolves.toBe(true);

    expect(applied).toHaveLength(1);
    expect(applied[0]?.[0]).toEqual({ type: "removeItems", itemIds: ["caption-1", "caption-2"] });
    expect(applied[0]?.map((action) => action.type)).not.toContain("createTrack");
    const captions = items(store).filter((item) => item.kind === "caption");
    expect(captions.map((item) => item.source)).toEqual([{ type: "text", text: "Make this sing" }]);
  });

  it("blocks regenerating cues on a locked track", async () => {
    const project = fixtureProject();
    project.timeline.tracks = project.timeline.tracks.map((track) => (track.kind === "caption" ? { ...track, locked: true } : track));
    const { store, applied, service } = setup(project, "");

    await expect(
      service.buildCaptions({ transcript, range: { startSeconds: 0, endSeconds: 2 }, wordsPerCue: 3, stylePreset: "boldReadableLower", replaceItemIds: ["caption-1"] }),
    ).resolves.toBe(false);

    expect(applied).toHaveLength(0);
    expect(store.getState().lastError).toBe("Unlock the caption track to regenerate captions.");
  });

  it("removes only the reviewed silence ranges in one ripple delete", async () => {
    const { applied, service } = setup(fixtureProject(), "");
    const reviewed = [{ startSeconds: 1, endSeconds: 1.5, trackIds: ["track-video"] }];

    await expect(service.removeSilences(reviewed)).resolves.toBe(true);
    await expect(service.removeSilences([])).resolves.toBe(true);

    expect(applied).toEqual([[{ type: "rippleDeleteRanges", ranges: reviewed }]]);
  });

  it("emits the denoise effect and preparation pair for every audio clip in one batch", async () => {
    const { store, applied, service } = setup(fixtureProject(), "");

    await expect(service.setDenoise(["music-bed"], true)).resolves.toBe(true);

    expect(applied).toEqual([
      [
        { type: "updateItemEffects", itemIds: ["music-bed"], effects: [expect.objectContaining({ effectType: "audio.denoise", params: { amount: 0.6 } })] },
        {
          type: "updateItemProperties",
          updates: [{ itemId: "music-bed", set: { audioDenoisePreparation: expect.objectContaining({ status: "queued" }) }, remove: [] }],
        },
      ],
    ]);
    await expect(service.setDenoise(["item-1"], true)).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Select an audio clip to change this property.");
  });

  describe("speakers", () => {
    function splitProject() {
      return { ...fixtureProject(), schemaVersion: 2 };
    }

    it("analyzes the clip's prepared audio, then reloads the project", async () => {
      const reloaded = { ...splitProject(), name: "Reloaded" };
      mockBackend({ analyze_project_speech: () => undefined, load_split_project_from_folder: () => reloaded });
      const project = splitProject();
      const audio = project.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === "music-bed");
      if (audio) audio.properties.audioDenoisePreparation = { status: "ready", artifact: "cache/voice.wav" };
      const { store, service } = setup(project);

      await expect(service.analyzeSpeakers("music-bed")).resolves.toBe(true);

      expect(backendRequest).toHaveBeenCalledWith("analyze_project_speech", {
        projectDir: "/projects/demo",
        mediaId: "media-voiceover",
        preparedPcmPath: "cache/voice.wav",
      });
      expect(store.getState().project.name).toBe("Reloaded");
    });

    it("uses the media file without a prepared artifact and marks a failed analysis", async () => {
      vi.mocked(backendRequest).mockImplementation(async (command: string) => {
        if (command === "analyze_project_speech") throw new Error("speech models are not installed");
        if (command === "apply_project_actions_to_split_project_folder") throw new BackendUnavailableError();
        return undefined;
      });
      const { store, service } = setup(splitProject());

      await expect(service.analyzeSpeakers("music-bed")).resolves.toBe(false);

      expect(backendRequest).toHaveBeenCalledWith("analyze_project_speech", expect.objectContaining({ preparedPcmPath: "media/voiceover.m4a" }));
      expect(items(store).find((item) => item.id === "music-bed")?.properties.speechAnalysis).toEqual({ status: "failed", quality: "production" });
      expect(store.getState().lastError).toBe("speech models are not installed");
    });

    it("renames a speaker with a trimmed name and ignores blank names", async () => {
      mockBackend({
        rename_project_speaker: (input) => ({ speakers: [{ id: input.speakerId, name: input.name, color: "#ff5a5a" }] }),
        load_split_project_from_folder: () => splitProject(),
      });
      const { service } = setup(splitProject());

      await expect(service.renameSpeaker("s1", "   ")).resolves.toBeNull();
      expect(backendRequest).not.toHaveBeenCalled();

      await expect(service.renameSpeaker("s1", "  Anna ")).resolves.toEqual([{ id: "s1", name: "Anna", color: "#ff5a5a" }]);
      expect(backendRequest).toHaveBeenCalledWith("rename_project_speaker", { projectDir: "/projects/demo", speakerId: "s1", name: "Anna" });
    });
  });
});
