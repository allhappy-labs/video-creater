import { captionBuildRangeForSourceItem, type CaptionBuildRange, type CaptionStylePreset } from "@/lib/captions/caption-items";
import { generatedTranscribeMediaJobId } from "@/lib/jobs/ids";
import { mediaContentKind } from "@/lib/media/media-filters";
import { mediaDisplayName } from "@/lib/media/names";
import type { ProjectAction, ProjectJobSummary, Transcript, VideoProject } from "@/lib/project";
import { captionPresetActions, captionStylePreset } from "@/lib/properties/caption-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { stringProperty, timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";
import { isReversedItem, localSecondsForSource, sourceSecondsAt, type SourceWindow } from "@/lib/timeline-ops/reverse";

type ItemPropertiesUpdate = Extract<ProjectAction, { type: "updateItemProperties" }>["updates"][number];

interface CaptionSourceOption {
  readonly value: string;
  readonly label: string;
}

/** Choices in the Generate captions card; the panel keeps them for Regenerate. */
export interface CaptionBuildSettings {
  readonly language: string;
  readonly maxWords: number;
  readonly censorProfanity: boolean;
}

export const defaultCaptionBuildSettings: CaptionBuildSettings = { language: "auto", maxWords: 4, censorProfanity: false };

/** Legacy caption language choices; "auto" lets the transcription model detect the language. */
export const captionLanguageOptions = [
  { value: "auto", label: "Auto" },
  { value: "en", label: "English" },
  { value: "de", label: "German" },
  { value: "fr", label: "French" },
  { value: "es", label: "Spanish" },
] as const;

export const captionMaxWordsOptions = Array.from({ length: 8 }, (_, index) => {
  const count = (index + 1).toString();
  return { value: count, label: count };
});

export type TranscriptionState = "idle" | "transcribing" | "failed";

const activeJobStatuses: ReadonlySet<string> = new Set(["queued", "running", "progress"]);
const failedJobStatuses: ReadonlySet<string> = new Set(["failed", "blocked"]);

function timelineItems(project: VideoProject): TimelineItem[] {
  return project.timeline.tracks.flatMap((track) => track.items);
}

/** Speech-capable media (video or audio content), in project order. */
export function captionSourceOptions(project: VideoProject): CaptionSourceOption[] {
  return project.media
    .filter((media) => {
      const kind = mediaContentKind(media);
      return kind === "video" || kind === "audio";
    })
    .map((media) => ({ value: media.id, label: mediaDisplayName(media) }));
}

export function transcriptForMedia(project: VideoProject, mediaId: string): Transcript | null {
  return project.transcripts.find((transcript) => transcript.mediaId === mediaId) ?? null;
}

/**
 * Legacy source rule: the selected caption's transcript media, else the selected clip's source,
 * else the first source with a transcript, else the first source.
 */
export function defaultCaptionSourceId(project: VideoProject, selectedItemIds: readonly string[]): string | null {
  const options = captionSourceOptions(project);
  const isOption = (mediaId: string | null | undefined): mediaId is string => options.some((option) => option.value === mediaId);
  const items = timelineItems(project);
  for (const itemId of selectedItemIds) {
    const item = items.find((candidate) => candidate.id === itemId);
    if (!item) continue;
    const transcriptId = item.kind === "caption" ? stringProperty(item, "transcriptId") : null;
    const mediaId = transcriptId
      ? project.transcripts.find((transcript) => transcript.id === transcriptId)?.mediaId
      : timelineItemSourceMediaId(item);
    if (isOption(mediaId)) return mediaId;
  }
  const transcribed = options.find((option) => transcriptForMedia(project, option.value));
  return transcribed?.value ?? options[0]?.value ?? null;
}

function jobMediaId(job: ProjectJobSummary): string | null {
  const mediaId = job.startRequest?.input.mediaId;
  return typeof mediaId === "string" ? mediaId : null;
}

/** A transcription job for the media: by its start request, or by the job id when there is none. */
function isTranscribeJobFor(job: ProjectJobSummary, mediaId: string): boolean {
  if (job.kind !== "transcribe_media") return false;
  const requested = jobMediaId(job);
  if (requested !== null) return requested === mediaId;
  const idPrefix = generatedTranscribeMediaJobId(mediaId).replace(/[^-]*$/, "");
  return job.id.startsWith(idPrefix);
}

function updatedAtMillis(job: ProjectJobSummary): number {
  const millis = Date.parse(job.updatedAt);
  return Number.isFinite(millis) ? millis : 0;
}

/** The latest transcription job's state for the media; a completed job is idle until the transcript loads. */
export function transcriptionState(project: VideoProject, mediaId: string): TranscriptionState {
  let latest: ProjectJobSummary | null = null;
  for (const job of project.jobs) {
    if (isTranscribeJobFor(job, mediaId) && (!latest || updatedAtMillis(job) >= updatedAtMillis(latest))) latest = job;
  }
  if (!latest) return "idle";
  if (activeJobStatuses.has(latest.status)) return "transcribing";
  return failedJobStatuses.has(latest.status) ? "failed" : "idle";
}

/** Timeline clips (not captions) whose source is the media, in timeline order. */
function sourceClips(project: VideoProject, mediaId: string): TimelineItem[] {
  return timelineItems(project)
    .filter((item) => item.kind !== "caption" && timelineItemSourceMediaId(item) === mediaId)
    .sort((left, right) => left.startSeconds - right.startSeconds);
}

/**
 * Legacy build range: the selected clip of the source, else its first clip (speed-aware source
 * range placed at the clip), else every transcript word at transcript time. Null without words.
 * Reversed clips play their words backwards, so they are never a build range: null when every clip
 * of the source is reversed.
 */
export function captionBuildRange(
  project: VideoProject,
  mediaId: string,
  transcript: Transcript,
  selectedItemIds: readonly string[],
): CaptionBuildRange | null {
  const clips = sourceClips(project, mediaId);
  const forwardClips = clips.filter((item) => !isReversedItem(item));
  const clip = forwardClips.find((item) => selectedItemIds.includes(item.id)) ?? forwardClips[0];
  if (clip) return captionBuildRangeForSourceItem(clip);
  if (clips.length > 0) return null;
  const first = transcript.words[0];
  const last = transcript.words[transcript.words.length - 1];
  return first && last ? { startSeconds: first.startSeconds, endSeconds: last.endSeconds } : null;
}

/** The clip's source window, with the speed its build range implies. */
function clipSourceWindow(clip: TimelineItem): SourceWindow {
  const range = captionBuildRangeForSourceItem(clip);
  const rate = range.timelineSecondsPerSourceSecond ?? 1;
  return { sourceIn: range.startSeconds, sourceOut: range.endSeconds, speed: rate > 0 ? 1 / rate : 1, reverse: isReversedItem(clip) };
}

/**
 * Transcript time under the playhead: through the clip of the source at the playhead, null between
 * its clips. A source with no clips uses the playhead directly, as the legacy transcript panel did.
 */
export function sourceSecondsAtPlayhead(project: VideoProject, mediaId: string, playheadSeconds: number): number | null {
  const clips = sourceClips(project, mediaId);
  if (clips.length === 0) return playheadSeconds;
  const clip = clips.find((item) => playheadSeconds >= item.startSeconds && playheadSeconds < item.startSeconds + item.durationSeconds);
  if (!clip) return null;
  return sourceSecondsAt(clipSourceWindow(clip), playheadSeconds - clip.startSeconds);
}

/** Timeline time for a transcript time: the first clip that shows it, else the transcript time itself. */
export function timelineSecondsForSource(project: VideoProject, mediaId: string, sourceSeconds: number): number {
  for (const clip of sourceClips(project, mediaId)) {
    const sourceWindow = clipSourceWindow(clip);
    if (sourceSeconds >= sourceWindow.sourceIn && sourceSeconds < sourceWindow.sourceOut) {
      return clip.startSeconds + localSecondsForSource(sourceWindow, sourceSeconds);
    }
  }
  return sourceSeconds;
}

/** Caption cues that came from the transcript, on the active timeline. */
export function transcriptCaptionItems(project: VideoProject, transcriptId: string): TimelineItem[] {
  return timelineItems(project).filter((item) => item.kind === "caption" && stringProperty(item, "transcriptId") === transcriptId);
}

function unlockedCaptions(project: VideoProject): TimelineItem[] {
  return project.timeline.tracks.filter((track) => !track.locked).flatMap((track) => track.items.filter((item) => item.kind === "caption"));
}

/** The style preset every unlocked caption shares, or null when they differ or there are none. */
export function sharedCaptionPreset(project: VideoProject): CaptionStylePreset | null {
  const presets = new Set(unlockedCaptions(project).map(captionStylePreset));
  const [preset] = presets;
  return presets.size === 1 && preset ? preset : null;
}

/**
 * The Styles grid: `captionPresetActions` (scope "all") for each caption group, merged into one
 * `updateItemProperties` batch over every unlocked caption.
 */
export function allCaptionsPresetActions(project: VideoProject, preset: CaptionStylePreset): CommandResult {
  const captions = unlockedCaptions(project);
  const unlockedIds = new Set(captions.map((item) => item.id));
  const updates = new Map<string, ItemPropertiesUpdate>();
  for (const caption of captions) {
    if (updates.has(caption.id)) continue;
    const result = captionPresetActions(project, caption.id, "all", preset);
    if ("blocked" in result) continue;
    for (const action of result.actions) {
      if (action.type !== "updateItemProperties") continue;
      for (const update of action.updates) {
        if (unlockedIds.has(update.itemId) && !updates.has(update.itemId)) updates.set(update.itemId, update);
      }
    }
  }
  if (updates.size === 0) return { blocked: "There are no unlocked captions to style." };
  return { actions: [{ type: "updateItemProperties", updates: [...updates.values()] }] };
}
