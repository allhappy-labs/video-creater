import { describe, expect, it } from "vitest";
import { captionBuildRangeForSourceItem } from "@/lib/captions/caption-items";
import { captionBuildActions } from "@/lib/captions/caption-track";
import type {
  ProjectAction,
  ProjectActionWriteResult,
  ProjectJobSummary,
  ProjectSpeakerRegistry,
  TemporalWorkflowStartRequest,
  TemporalWorkflowStartResult,
  VideoProject,
} from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { createFixtureProjectStore } from "./fixture-project-store";
import { projectFixtureOperations } from "./project-fixtures";
import { speechFixtureOperations, transcriptionFixturePolls, untranscribedSample } from "./speech-fixtures";

const projectDir = "/tmp/video-creater-editor-project";
const jobId = "transcribe-media-media-1-fixture";

function setup() {
  const store = createFixtureProjectStore();
  store.addSeed(untranscribedSample);
  const handlers = new Map([...projectFixtureOperations(store), ...speechFixtureOperations(store)]);
  async function request<Result>(operation: string, input: Record<string, unknown> = {}): Promise<Result> {
    const handler = handlers.get(operation);
    if (!handler) throw new Error(`no speech fixture handler for ${operation}`);
    return structuredClone(await handler(structuredClone(input))) as Result;
  }
  const reload = () => request<VideoProject>("load_split_project_from_folder", { projectDir });
  const apply = async (actions: ProjectAction[]) => (await request<ProjectActionWriteResult>("apply_project_actions_to_split_project_folder", { projectDir, actions })).project;
  return { store, request, reload, apply };
}

function jobStatus(project: VideoProject): string | undefined {
  return project.jobs.find((job) => job.id === jobId)?.status;
}

/** The speech service's transcribe(): record the queued job, start the workflow, record the start. */
async function transcribe({ request, apply }: ReturnType<typeof setup>, project: VideoProject): Promise<VideoProject> {
  const updatedAt = "2026-09-15T10:00:00.000Z";
  const job = await request<ProjectJobSummary>("build_temporal_job_summary", { kind: "transcribe_media", projectId: project.id, jobId, status: "queued", updatedAt });
  const startRequest = await request<TemporalWorkflowStartRequest>("build_temporal_transcribe_media_start_request", { projectId: project.id, projectDir, mediaId: "media-1", jobId, languageMode: "auto" });
  const queuedJob = { ...job, startRequest };
  const queued = await apply([{ type: "recordJob", job: queuedJob }]);
  expect(jobStatus(queued)).toBe("queued");
  const started = await request<TemporalWorkflowStartResult>("start_temporal_workflow", { job: queuedJob });
  expect(started).toMatchObject({ status: "started", workflowId: startRequest.workflowId });
  const action = await request<ProjectAction>("build_temporal_start_result_action", { job: queuedJob, runId: started.runId, updatedAt });
  return apply([action]);
}

describe("speech fixture operations", () => {
  it("opens the sample untranscribed, without the cues built from its transcript", async () => {
    const { request } = setup();
    const { project } = await request<ProjectActionWriteResult>("save_split_project_to_folder", { projectDir, project: createSampleProject(), expectedRevision: 0 });
    expect(project.transcripts).toEqual([]);
    expect(project.timeline.tracks.flatMap((track) => track.items).filter((item) => item.kind === "caption")).toEqual([]);
    expect(project.timeline.tracks.some((track) => track.kind === "caption")).toBe(true);
  });

  it("runs a transcription queued, then running, then completed across folder reloads, storing its words", async () => {
    const context = setup();
    const { project } = await context.request<ProjectActionWriteResult>("save_split_project_to_folder", { projectDir, project: createSampleProject(), expectedRevision: 0 });
    const running = await transcribe(context, project);
    expect(jobStatus(running)).toBe("running");
    expect(transcriptionFixturePolls).toBe(2);

    const first = await context.reload();
    expect(jobStatus(first)).toBe("running");
    expect(first.transcripts).toEqual([]);
    const second = await context.reload();
    expect(jobStatus(second)).toBe("completed");
    const transcript = second.transcripts.find((candidate) => candidate.mediaId === "media-1");
    expect(transcript?.id).toBe("transcript-media-1");
    expect(transcript?.words.map((word) => word.text).join(" ")).toBe("The restored newsreel plays at its original speed");
    expect(transcript?.words.some((word) => (word.confidence ?? 1) < 0.7)).toBe(true);
    // Worker bookkeeping keeps the revision, so the editor's next save isn't stale.
    expect(second.contentRevision).toBe(running.contentRevision);

    // The editor builds the cues from the stored transcript, as in the desktop app.
    const clip = second.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === "item-1");
    if (!transcript || !clip) throw new Error("the transcript and the opening clip exist");
    const plan = captionBuildActions(second, { transcript, range: captionBuildRangeForSourceItem(clip), wordsPerCue: 4, stylePreset: "boldReadableLower", groupId: "caption-group-1" }, "track-captions-2");
    if ("blocked" in plan) throw new Error(plan.blocked);
    const captioned = await context.apply(plan.actions);
    const cues = captioned.timeline.tracks.flatMap((track) => track.items).filter((item) => item.kind === "caption");
    expect(cues.map((cue) => (cue.source.type === "text" ? cue.source.text : ""))).toEqual(["The restored newsreel plays", "at its original speed"]);
    expect(cues.every((cue) => cue.properties.transcriptId === "transcript-media-1")).toBe(true);
  });

  it("reports other workflows unavailable, like a build without the Temporal worker", async () => {
    const { request } = setup();
    const job = await request<ProjectJobSummary>("build_temporal_job_summary", { kind: "export_media", projectId: "project-sample", jobId: "export-1", status: "queued", updatedAt: "2026-09-15T10:00:00.000Z" });
    await expect(request<TemporalWorkflowStartResult>("start_temporal_workflow", { job })).resolves.toMatchObject({ status: "unavailable", runId: null });
  });

  it("reads and renames the speaker the transcript words name", async () => {
    const { request } = setup();
    const registry = await request<ProjectSpeakerRegistry>("get_project_speaker_registry", { projectDir });
    expect(registry.speakers).toEqual([{ id: "fixture-speaker-1", name: "Speaker 1", color: "#4f8ef7" }]);
    const renamed = await request<ProjectSpeakerRegistry>("rename_project_speaker", { projectDir, speakerId: "fixture-speaker-1", name: "  Narrator " });
    expect(renamed.speakers?.[0]?.name).toBe("Narrator");
    await expect(request<ProjectSpeakerRegistry>("get_project_speaker_registry", { projectDir })).resolves.toMatchObject({ speakers: [{ name: "Narrator" }] });
    await expect(request("rename_project_speaker", { projectDir, speakerId: "missing", name: "Host" })).rejects.toBe("speaker id was not found");
    await expect(request("rename_project_speaker", { projectDir, speakerId: "fixture-speaker-1", name: " " })).rejects.toBe("speaker name must not be blank");
  });
});
