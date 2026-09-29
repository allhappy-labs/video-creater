import type { ProjectAction, Transcript, VideoProject } from "@/lib/project";
import { getTimelineItemText, type TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { numberProperty, stringProperty } from "@/lib/timeline-ops/item-properties";

interface TranscriptWordFix {
  readonly transcriptId: string;
  readonly wordIndex: number;
  readonly text: string;
}

interface WordRange {
  readonly start: number;
  readonly end: number;
}

const overlapToleranceSeconds = 0.001;

/** Letters and digits; anything else around a word counts as punctuation. */
const punctuationAffixes = /^([^\p{L}\p{N}]*)(.*?)([^\p{L}\p{N}]*)$/u;

function wordCore(token: string): string {
  return (punctuationAffixes.exec(token)?.[2] ?? token).toLocaleLowerCase();
}

function isWordIndex(value: number | null): value is number {
  return value !== null && Number.isInteger(value) && value >= 0;
}

/**
 * The transcript words a cue shows: built cues store `wordStartIndex`/`wordEndIndex`; otherwise the
 * words overlapping its `sourceIn`–`sourceOut`; otherwise a repair cue's single `wordIndex`.
 */
function cueWordRange(transcript: Transcript, item: TimelineItem): WordRange | null {
  const start = numberProperty(item, "wordStartIndex");
  const end = numberProperty(item, "wordEndIndex");
  if (isWordIndex(start) && isWordIndex(end) && end >= start) return { start, end };

  const sourceIn = numberProperty(item, "sourceIn");
  const sourceOut = numberProperty(item, "sourceOut");
  if (sourceIn !== null && sourceOut !== null && sourceOut > sourceIn) {
    const indices = transcript.words.flatMap((word, index) =>
      word.endSeconds > sourceIn + overlapToleranceSeconds && word.startSeconds < sourceOut - overlapToleranceSeconds ? [index] : [],
    );
    const first = indices[0];
    const last = indices[indices.length - 1];
    return first !== undefined && last !== undefined ? { start: first, end: last } : null;
  }

  const wordIndex = numberProperty(item, "wordIndex");
  return isWordIndex(wordIndex) ? { start: wordIndex, end: wordIndex } : null;
}

/** Replaces the old word inside a cue token, keeping the token's surrounding punctuation. */
function replaceToken(token: string, before: string, after: string): string {
  if (token === before) return after;
  const match = punctuationAffixes.exec(token);
  if (match && wordCore(token) === wordCore(before)) return `${match[1] ?? ""}${after}${match[3] ?? ""}`;
  return after;
}

/**
 * The cue text with the fixed word, or null when the cue no longer shows it. When the cue has one
 * token per transcript word, the word is replaced by position; otherwise (edited or repaired text)
 * the token matching the old word closest to that position is replaced.
 */
function cueTextWithFix(text: string, range: WordRange, wordIndex: number, before: string, after: string): string | null {
  const parts = text.split(/(\s+)/);
  const tokenParts = parts.flatMap((part, index) => (part.trim().length > 0 ? [index] : []));
  const offset = wordIndex - range.start;
  let target: number | undefined;
  if (tokenParts.length === range.end - range.start + 1) {
    target = tokenParts[offset];
  } else {
    const matches = tokenParts
      .map((partIndex, tokenIndex) => ({ partIndex, distance: Math.abs(tokenIndex - offset) }))
      .filter(({ partIndex }) => wordCore(parts[partIndex] ?? "") === wordCore(before))
      .sort((left, right) => left.distance - right.distance);
    target = matches[0]?.partIndex;
  }
  if (target === undefined) return null;
  const token = parts[target];
  if (token === undefined) return null;
  parts[target] = replaceToken(token, before, after);
  return parts.join("");
}

/**
 * Double-click word fix as one batch: `editTranscriptWords` for the word (text only, timing kept),
 * plus `editCaptionText` for every cue built from that word. `applyCaptionRepair` is not used: it
 * rewrites a single cue to the word and moves the cue to the word's timing.
 */
export function transcriptWordFixActions(project: VideoProject, fix: TranscriptWordFix, now = new Date()): CommandResult {
  const transcript = project.transcripts.find((candidate) => candidate.id === fix.transcriptId);
  if (!transcript) return { blocked: "That transcript is no longer in the project." };
  const word = transcript.words[fix.wordIndex];
  if (!word) return { blocked: "That word is no longer in the transcript." };
  const text = fix.text.trim();
  if (text.length === 0) return { blocked: "A word can't be empty." };
  if (text === word.text) return { actions: [] };

  const actions: ProjectAction[] = [
    {
      type: "editTranscriptWords",
      edits: [
        {
          transcriptId: transcript.id,
          wordIndex: fix.wordIndex,
          text,
          repairId: `word-repair-${transcript.id}-${fix.wordIndex.toString()}-${now.getTime().toString(36)}`,
          createdAt: now.toISOString(),
        },
      ],
    },
  ];
  for (const track of project.timeline.tracks) {
    for (const item of track.items) {
      if (item.kind !== "caption" || stringProperty(item, "transcriptId") !== transcript.id) continue;
      const range = cueWordRange(transcript, item);
      if (!range || fix.wordIndex < range.start || fix.wordIndex > range.end) continue;
      const current = getTimelineItemText(item);
      const next = cueTextWithFix(current, range, fix.wordIndex, word.text, text);
      if (next === null || next === current) continue;
      if (track.locked) return { blocked: "Unlock the caption track to fix this word." };
      actions.push({ type: "editCaptionText", itemId: item.id, text: next });
    }
  }
  return { actions };
}
