import {
  buildFallbackTemporalStartResultAction,
  buildFallbackTranscribeMediaStartRequest,
  temporalIdSegment,
  temporalWorkflowDefinitions,
  type TemporalWorkflowKind,
} from "../../jobs/temporal-fallback";
import type {
  ProjectJobStatus,
  ProjectJobSummary,
  ProjectSpeakerIdentity,
  TemporalWorkflowStartResult,
  Transcript,
  VideoProject,
} from "../../project";
import type { FixtureOperationHandler } from "../adapters/fixture-transport";
import type { FixtureProjectStore } from "./fixture-project-store";

/**
 * DEV-only speech handlers over the shared fixture project store:
 *
 * - The Temporal job commands the speech service calls (`build_temporal_job_summary`, the transcription
 *   start request, `start_temporal_workflow`, `build_temporal_start_result_action`), answered with the
 *   same shapes as the TypeScript fallbacks of the native commands.
 * - A started transcription runs on the store's job clock: the first folder reload marks the job
 *   running, the second completes it and stores a deterministic transcript for the media (with one
 *   low-confidence word to check), as the worker's StoreTranscript activity does. Captions are then
 *   built by the editor from the transcript, as in the desktop app.
 * - The speaker registry, with one speaker the transcript words name, can be read and renamed.
 *
 * Other workflow kinds report the Temporal runtime unavailable, like a build without the worker.
 */

/** Folder reloads a transcription takes: running on the first, completed on the second. */
export const transcriptionFixturePolls = 2;

const fixtureSpeaker: ProjectSpeakerIdentity = { id: "fixture-speaker-1", name: "Speaker 1", color: "#4f8ef7" };

/** Words spoken over the first four seconds of a source; "newsreel" is the low-confidence word. */
const spokenWords = [
  { text: "The", startSeconds: 0.4, endSeconds: 0.62, confidence: 0.97 },
  { text: "restored", startSeconds: 0.66, endSeconds: 1.1, confidence: 0.95 },
  { text: "newsreel", startSeconds: 1.14, endSeconds: 1.7, confidence: 0.58 },
  { text: "plays", startSeconds: 1.76, endSeconds: 2.1, confidence: 0.96 },
  { text: "at", startSeconds: 2.3, endSeconds: 2.42, confidence: 0.98 },
  { text: "its", startSeconds: 2.46, endSeconds: 2.62, confidence: 0.97 },
  { text: "original", startSeconds: 2.66, endSeconds: 3.14, confidence: 0.94 },
  { text: "speed", startSeconds: 3.18, endSeconds: 3.6, confidence: 0.96 },
] as const;

const temporalUnavailableMessage =
  "Temporal runtime is unavailable in this build. Rebuild with default features or explicitly pass --features temporal-worker, run `temporal server start-dev`, and start the worker before dispatching workflow executions.";

/** The transcript the fixture worker stores: `transcript-<mediaId>`, fitted inside short sources. */
function fixtureTranscript(media: Pick<VideoProject["media"][number], "id" | "durationSeconds">): Transcript {
  const scale = media.durationSeconds > 0 && media.durationSeconds < 4 ? media.durationSeconds / 4 : 1;
  const words = spokenWords.map((word) => ({
    text: word.text,
    startSeconds: Math.round(word.startSeconds * scale * 1000) / 1000,
    endSeconds: Math.round(word.endSeconds * scale * 1000) / 1000,
    confidence: word.confidence,
    speaker: fixtureSpeaker.id,
  }));
  const segment = (from: number, to: number) => {
    const slice = words.slice(from, to);
    return { text: slice.map((word) => word.text).join(" "), startSeconds: slice[0]?.startSeconds ?? 0, endSeconds: slice[slice.length - 1]?.endSeconds ?? 0 };
  };
  return { id: `transcript-${media.id}`, mediaId: media.id, engine: "fixture", rawArtifactPath: null, repairs: [], segments: [segment(0, 4), segment(4, 8)], words };
}

/**
 * The bundled sample before anything was transcribed: no transcripts, and none of the caption cues
 * built from them, so Captions opens on Generate captions. Other projects are left alone.
 */
export function untranscribedSample(project: VideoProject): VideoProject {
  if (project.id !== "project-sample" || project.transcripts.length === 0) return project;
  const transcriptIds = new Set(project.transcripts.map((transcript) => transcript.id));
  const fromTranscript = (item: VideoProject["timeline"]["tracks"][number]["items"][number]) =>
    item.kind === "caption" && typeof item.properties.transcriptId === "string" && transcriptIds.has(item.properties.transcriptId);
  return {
    ...project,
    transcripts: [],
    timeline: { ...project.timeline, tracks: project.timeline.tracks.map((track) => ({ ...track, items: track.items.filter((item) => !fromTranscript(item)) })) },
  };
}

function jobSummary(input: Record<string, unknown>): ProjectJobSummary {
  const { kind, projectId, jobId, status, updatedAt } = input as { kind: TemporalWorkflowKind; projectId: string; jobId: string; status: ProjectJobStatus; updatedAt: string };
  const definition = temporalWorkflowDefinitions[kind];
  return {
    id: jobId,
    kind,
    status,
    updatedAt,
    workflow: {
      workflowId: `video-creater/${temporalIdSegment(projectId)}/${definition.segment}/${temporalIdSegment(jobId)}`,
      workflowType: definition.workflowType,
      taskQueue: "video-creater-workflows",
      runId: null,
      activityTypes: [...definition.activityTypes],
    },
  };
}

function withJobStatus(project: VideoProject, jobId: string, status: ProjectJobStatus, updatedAt: string): VideoProject {
  return { ...project, jobs: project.jobs.map((job) => (job.id === jobId ? { ...job, status, updatedAt } : job)) };
}

interface PendingTranscription {
  readonly jobId: string;
  readonly mediaId: string;
  polls: number;
}

export function speechFixtureOperations(store: FixtureProjectStore): ReadonlyMap<string, FixtureOperationHandler> {
  const pending = new Map<string, PendingTranscription>();
  const speakers: ProjectSpeakerIdentity[] = [{ ...fixtureSpeaker }];
  const registry = () => ({ schemaVersion: 1, analyzerVersion: "fixture", embeddingVersion: "fixture", speakers: speakers.map((speaker) => ({ ...speaker })) });

  /** One folder reload: each transcription runs, then stores its transcript and completes. */
  store.onReload(() => {
    for (const run of [...pending.values()]) {
      const project = store.require();
      const media = project.media.find((candidate) => candidate.id === run.mediaId);
      if (!media || !project.jobs.some((job) => job.id === run.jobId)) {
        pending.delete(run.jobId);
        continue;
      }
      run.polls += 1;
      const now = new Date().toISOString();
      if (run.polls < transcriptionFixturePolls) {
        store.record(withJobStatus(project, run.jobId, "running", now));
        continue;
      }
      pending.delete(run.jobId);
      const transcript = fixtureTranscript(media);
      store.record({
        ...withJobStatus(project, run.jobId, "completed", now),
        transcripts: [...project.transcripts.filter((candidate) => candidate.mediaId !== media.id), transcript],
      });
    }
  });

  function startWorkflow(input: Record<string, unknown>): TemporalWorkflowStartResult {
    const { job } = input as { job: ProjectJobSummary };
    const request = job.startRequest;
    const identity = { workflowId: request?.workflowId ?? "", workflowType: request?.workflowType ?? "", taskQueue: request?.taskQueue ?? "" };
    const mediaId = request?.input.mediaId;
    if (job.kind !== "transcribe_media" || typeof mediaId !== "string") {
      return { status: "unavailable", ...identity, runId: null, message: temporalUnavailableMessage };
    }
    pending.set(job.id, { jobId: job.id, mediaId, polls: 0 });
    return { status: "started", ...identity, runId: `fixture-run-${temporalIdSegment(job.id)}`, message: "Workflow started." };
  }

  return new Map<string, FixtureOperationHandler>([
    ["build_temporal_job_summary", jobSummary],
    ["build_temporal_transcribe_media_start_request", (input) => buildFallbackTranscribeMediaStartRequest(input as Parameters<typeof buildFallbackTranscribeMediaStartRequest>[0])],
    ["build_temporal_start_result_action", (input) => buildFallbackTemporalStartResultAction(input as Parameters<typeof buildFallbackTemporalStartResultAction>[0])],
    ["start_temporal_workflow", startWorkflow],
    ["get_project_speaker_registry", registry],
    [
      "rename_project_speaker",
      (input) => {
        const { speakerId, name } = input as { speakerId: string; name: string };
        if (!name.trim()) throw "speaker name must not be blank";
        const speaker = speakers.find((candidate) => candidate.id === speakerId);
        if (!speaker) throw "speaker id was not found";
        speaker.name = name.trim();
        return registry();
      },
    ],
  ]);
}
