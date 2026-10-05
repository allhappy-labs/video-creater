import { useMemo } from "react";
import type { CaptionBuildOptions } from "@/lib/captions/caption-items";
import { captionBuildActions } from "@/lib/captions/caption-track";
import { generatedTranscribeMediaJobId } from "@/lib/jobs/ids";
import { workflowStartFailureActions } from "@/lib/jobs/start-failure";
import { buildFallbackTranscribeMediaStartRequest, buildTemporalJobSummary } from "@/lib/jobs/temporal-fallback";
import {
  analyzeProjectSpeech,
  applyProjectActionLocally,
  buildTemporalStartResultAction,
  buildTemporalTranscribeMediaStartRequest,
  loadSplitProjectFromFolder,
  renameProjectSpeaker,
  runTranscribeMediaInProcess,
  startTemporalWorkflow,
  type ProjectAction,
  type ProjectActionRippleDeleteRange,
  type ProjectJobSummary,
  type ProjectSpeakerIdentity,
  type TemporalWorkflowStartResult,
  type TemporalWorkflowStartRequest,
  type VideoProject,
} from "@/lib/project";
import { loadAppSettingsPreferences, type AppSettingsPreferences } from "@/lib/app-settings";
import { audioDenoise, denoiseActions } from "@/lib/properties/audio-properties";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { TimelineItem } from "@/lib/timeline";
import { newTrackId } from "@/lib/timeline-ops/dynamic-tracks";
import { timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";
import type { EditorStore } from "../store/editor-store";
import { useEditorStoreApi } from "../store/editor-store-context";

/** Caption build input; the service assigns the caption group id. */
interface BuildCaptionsInput extends Omit<CaptionBuildOptions, "groupId"> {
  /** Masks common profanity in the cue text (the transcript keeps the spoken words). */
  readonly censorProfanity?: boolean;
  /** Existing cues the new captions replace (Regenerate), removed in the same batch. */
  readonly replaceItemIds?: readonly string[];
}

/**
 * Store-bound speech work: transcription, caption building, silence removal, denoise and speakers.
 * Each method resolves `false` when it was blocked or failed, with the reason in `lastError`.
 */
export interface SpeechService {
  /**
   * Records the transcription job, then runs it: in the backend process by default (resolving once
   * the transcript is stored), or as a Temporal workflow when that execution backend is selected.
   */
  transcribe(mediaId: string, languageMode?: string): Promise<boolean>;
  /**
   * Places built captions (a caption track when needed, plus the cues) as one undo step and selects them.
   * `replaceItemIds` are removed in that same step, before the cues are placed.
   */
  buildCaptions(input: BuildCaptionsInput): Promise<boolean>;
  /** Ripple-deletes exactly the reviewed ranges as one undo step; an empty list is a no-op. */
  removeSilences(ranges: readonly ProjectActionRippleDeleteRange[]): Promise<boolean>;
  /** Turns denoise on or off for audio clips in one batch, keeping each clip's strength. */
  setDenoise(itemIds: readonly string[], enabled: boolean): Promise<boolean>;
  /** Pre-cut `analyzeItemSpeech`: on-device speech analysis of the clip's audio, then a project reload. */
  analyzeSpeakers(itemId: string): Promise<boolean>;
  /** Renames with the trimmed name; resolves the updated speakers, or null when ignored or failed. */
  renameSpeaker(speakerId: string, name: string): Promise<readonly ProjectSpeakerIdentity[] | null>;
}

interface TranscribeStartInput {
  readonly projectId: string;
  readonly projectDir: string;
  readonly mediaId: string;
  readonly jobId: string;
  readonly languageMode: string;
}

/** Words "Censor profanity" masks, compared case-insensitively without surrounding punctuation. */
const profaneWords: ReadonlySet<string> = new Set([
  "asshole",
  "bastard",
  "bitch",
  "bullshit",
  "cock",
  "cunt",
  "dick",
  "fuck",
  "fucked",
  "fucker",
  "fucking",
  "motherfucker",
  "shit",
  "shitty",
  "twat",
  "wanker",
]);

/** Keeps each profane word's first letter and masks the rest: "Shit!" becomes "S***!". */
function censorCaptionText(text: string): string {
  return text.replace(/[\p{L}']+/gu, (word) =>
    profaneWords.has(word.toLocaleLowerCase()) ? `${word.slice(0, 1)}${"*".repeat(word.length - 1)}` : word,
  );
}

function censorAddedCaptions(action: ProjectAction): ProjectAction {
  if (action.type !== "addItems") return action;
  return {
    ...action,
    items: action.items.map((item) =>
      item.source.type === "text" ? { ...item, source: { ...item.source, text: censorCaptionText(item.source.text) } } : item,
    ),
  };
}

/** Jobs whose workflow start is in flight, per store, so a double click never starts twice. */
const startingJobIdsByStore = new WeakMap<EditorStore, Set<string>>();

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function findItem(project: VideoProject, itemId: string): TimelineItem | null {
  for (const track of project.timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return item;
  }
  return null;
}

/** The denoised PCM artifact prepared for the clip, when there is one. */
function preparedAudioArtifact(item: TimelineItem): string | null {
  const preparation = item.properties.audioDenoisePreparation;
  if (!preparation || typeof preparation !== "object" || Array.isArray(preparation)) return null;
  const artifact = (preparation as Record<string, unknown>).artifact;
  return typeof artifact === "string" ? artifact : null;
}

function speakersOf(registry: unknown): ProjectSpeakerIdentity[] {
  const speakers = (registry as { speakers?: unknown } | null | undefined)?.speakers;
  return Array.isArray(speakers) ? (speakers as ProjectSpeakerIdentity[]) : [];
}

function isProject(value: unknown): value is VideoProject {
  return typeof value === "object" && value !== null && "timeline" in value && "media" in value;
}

async function transcribeStartRequest(input: TranscribeStartInput): Promise<TemporalWorkflowStartRequest> {
  try {
    return await buildTemporalTranscribeMediaStartRequest({ ...input });
  } catch (error) {
    if (!isBackendUnavailableError(error)) throw error;
    return buildFallbackTranscribeMediaStartRequest({ ...input });
  }
}

interface SpeechServiceOptions {
  /** Defaults to the accepted app preferences. */
  readonly preferences?: () => AppSettingsPreferences;
}

export function createSpeechService(store: EditorStore, options: SpeechServiceOptions = {}): SpeechService {
  const state = () => store.getState();
  const preferences = options.preferences ?? loadAppSettingsPreferences;
  let startingJobIds = startingJobIdsByStore.get(store);
  if (!startingJobIds) {
    startingJobIds = new Set();
    startingJobIdsByStore.set(store, startingJobIds);
  }
  const starting = startingJobIds;

  function block(reason: string): false {
    state().setLastError(reason);
    return false;
  }

  /** The workflow never started: fail the job so the task doesn't stay queued, then say why. */
  async function failStart(job: ProjectJobSummary, reason: string): Promise<false> {
    await state().applyActions(workflowStartFailureActions(job, new Date().toISOString()));
    return block(reason);
  }

  /** Pre-cut `startQueuedTemporalWorkflow` without the generation poller. */
  async function startQueuedWorkflow(job: ProjectJobSummary): Promise<boolean> {
    if (!job.startRequest || starting.has(job.id)) return true;
    starting.add(job.id);
    try {
      let result: TemporalWorkflowStartResult;
      try {
        result = await startTemporalWorkflow({ job });
      } catch (error) {
        return await failStart(job, errorMessage(error));
      }
      if (result.status !== "started" || !result.runId) return await failStart(job, result.message);
      const action = await buildTemporalStartResultAction({ job, runId: result.runId, updatedAt: new Date().toISOString() });
      return (await state().applyActions([action])) !== null;
    } catch (error) {
      return block(errorMessage(error));
    } finally {
      starting.delete(job.id);
    }
  }

  /**
   * Runs the queued job in the backend process and waits for it. The backend marks the job running,
   * stores the transcript and completes or fails the job; polling shows the running state meanwhile.
   */
  async function runQueuedJobInProcess(job: ProjectJobSummary): Promise<boolean> {
    if (!job.startRequest || starting.has(job.id)) return true;
    starting.add(job.id);
    try {
      state().startPolling();
      const completed: unknown = await runTranscribeMediaInProcess({ startRequest: job.startRequest, updatedAt: new Date().toISOString() });
      if (isProject(completed)) await state().mergeLoadedProject(completed);
      return true;
    } catch (error) {
      if (isBackendUnavailableError(error)) {
        // Bridges without the native command fall back to the distributed Temporal start.
        starting.delete(job.id);
        return await startQueuedWorkflow(job);
      }
      // The backend fails the job with its reason before rejecting, so reload to show it.
      await reloadProject();
      return block(errorMessage(error));
    } finally {
      starting.delete(job.id);
    }
  }

  /**
   * Speech commands write the split project on the backend, so the project is reloaded afterwards.
   * The reload is not an undo step: undo would overwrite the backend's analysis results.
   */
  async function reloadProject(): Promise<void> {
    try {
      const project = await loadSplitProjectFromFolder({ projectDir: state().projectDir });
      if (isProject(project)) state().replaceProject(project);
    } catch (error) {
      if (!isBackendUnavailableError(error)) state().setLastError(errorMessage(error));
    }
  }

  return {
    async transcribe(mediaId, languageMode = "auto") {
      const { project, projectDir } = state();
      if (!project.media.some((media) => media.id === mediaId)) return block("That media is no longer in the project.");
      const jobId = generatedTranscribeMediaJobId(mediaId);
      state().setLastError(null);
      try {
        const job = await buildTemporalJobSummary("transcribe_media", project.id, jobId, new Date().toISOString());
        const startRequest = await transcribeStartRequest({ projectId: project.id, projectDir, mediaId, jobId, languageMode });
        const queuedJob: ProjectJobSummary = { ...job, startRequest };
        const recorded = await state().applyActions([{ type: "recordJob", job: queuedJob }]);
        if (!recorded) return block("Transcription workflow could not be recorded.");
        if (preferences().generationExecutionBackend === "temporal") return await startQueuedWorkflow(queuedJob);
        return await runQueuedJobInProcess(queuedJob);
      } catch (error) {
        return block(errorMessage(error));
      }
    },

    async buildCaptions({ censorProfanity = false, replaceItemIds = [], ...options }) {
      const { project } = state();
      const replaced = new Set(replaceItemIds);
      const removal: ProjectAction[] = replaced.size > 0 ? [{ type: "removeItems", itemIds: [...replaced] }] : [];
      if (project.timeline.tracks.some((track) => track.locked && track.items.some((item) => replaced.has(item.id)))) {
        return block("Unlock the caption track to regenerate captions.");
      }
      // Placement is planned against the timeline without the replaced cues, so they free their track.
      const base = removal.reduce(applyProjectActionLocally, project);
      const groupId = `caption-group-${Date.now().toString(36)}`;
      const plan = captionBuildActions(base, { ...options, groupId }, newTrackId(base.timeline, "caption"));
      if ("blocked" in plan) return block(plan.blocked);
      const placed = censorProfanity ? plan.actions.map(censorAddedCaptions) : plan.actions;
      if (!(await state().applyActions([...removal, ...placed]))) return false;
      state().selectItems(plan.itemIds);
      return true;
    },

    async removeSilences(ranges) {
      if (ranges.length === 0) return true;
      const copies = ranges.map((range) => ({ ...range, trackIds: [...range.trackIds] }));
      return (await state().applyActions([{ type: "rippleDeleteRanges", ranges: copies }])) !== null;
    },

    async setDenoise(itemIds, enabled) {
      const { project } = state();
      const actions: ProjectAction[] = [];
      for (const itemId of itemIds) {
        const item = findItem(project, itemId);
        if (!item) return block("That clip is no longer on the timeline.");
        const result = denoiseActions(item, enabled, audioDenoise(item).amount);
        if ("blocked" in result) return block(result.blocked);
        actions.push(...result.actions);
      }
      if (actions.length === 0) return true;
      return (await state().applyActions(actions)) !== null;
    },

    async analyzeSpeakers(itemId) {
      const { project, projectDir } = state();
      const item = findItem(project, itemId);
      const mediaId = item ? timelineItemSourceMediaId(item) : null;
      const media = project.media.find((candidate) => candidate.id === mediaId);
      if (!item || !mediaId || !media) return block("Select a clip with audio to analyze.");
      if (projectDir.trim().length === 0) return block("Save the project before analyzing speech.");
      state().setLastError(null);
      try {
        await analyzeProjectSpeech({ projectDir, mediaId, preparedPcmPath: preparedAudioArtifact(item) ?? media.relativePath });
      } catch (error) {
        await state().applyActions([
          {
            type: "updateItemProperties",
            updates: [{ itemId, set: { speechAnalysis: { status: "failed", quality: "production" } }, remove: [] }],
          },
        ]);
        // applyActions clears lastError, so the analysis failure is shown after it.
        return block(errorMessage(error));
      }
      await reloadProject();
      return true;
    },

    async renameSpeaker(speakerId, name) {
      const trimmed = name.trim();
      const { projectDir } = state();
      if (!trimmed || projectDir.trim().length === 0) return null;
      try {
        const speakers = speakersOf(await renameProjectSpeaker({ projectDir, speakerId, name: trimmed }));
        await reloadProject();
        return speakers;
      } catch (error) {
        state().setLastError(errorMessage(error));
        return null;
      }
    },
  };
}

/** The speech service bound to the editor store; stable for the lifetime of the store. */
export function useSpeechService(): SpeechService {
  const store = useEditorStoreApi();
  return useMemo(() => createSpeechService(store), [store]);
}
