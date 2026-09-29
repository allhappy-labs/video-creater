import type { VideoProject } from "@/lib/project";

/** Gaps between consecutive words of one speaker at or above this length show a pause chip. */
const pauseChipMinSeconds = 0.6;
/** Words whose recognizer confidence is below this are marked for checking. */
const lowConfidenceThreshold = 0.6;

interface TranscriptWordSpan {
  readonly isPause: false;
  /** Index into `transcript.words`; used for seeking, repairs and `data-word-index`. */
  readonly index: number;
  readonly text: string;
  readonly startSeconds: number;
  readonly endSeconds: number;
  /** Present only when the transcript word carries a numeric confidence. */
  readonly confidence?: number;
  readonly lowConfidence: boolean;
}

interface TranscriptPauseSpan {
  readonly isPause: true;
  /** Index of the transcript word that follows the pause. */
  readonly index: number;
  readonly startSeconds: number;
  readonly endSeconds: number;
  readonly durationSeconds: number;
}

type TranscriptSpan = TranscriptWordSpan | TranscriptPauseSpan;

export interface TranscriptParagraph {
  readonly id: string;
  /** Raw diarization label; map it to a registry name before display. Null when unlabelled. */
  readonly speakerId: string | null;
  /** 1-based order of the speaker's first appearance, for "Speaker N" fallbacks. */
  readonly speakerNumber: number | null;
  readonly startSeconds: number;
  readonly endSeconds: number;
  readonly spans: readonly TranscriptSpan[];
}

interface MutableParagraph {
  id: string;
  speakerId: string | null;
  speakerNumber: number | null;
  startSeconds: number;
  endSeconds: number;
  spans: TranscriptSpan[];
}

function roundMilliseconds(seconds: number) {
  return Math.round(seconds * 1000) / 1000;
}

/**
 * Speaker-labelled paragraphs for the transcript of `mediaId`. A new paragraph starts when the
 * speaker label changes; unlabelled words stay with the current paragraph. Inside a paragraph,
 * gaps of 0.6 s or more become pause spans. Words with a numeric confidence below 0.6 are marked.
 */
export function transcriptParagraphs(project: VideoProject, mediaId: string): TranscriptParagraph[] {
  const transcript = project.transcripts.find((candidate) => candidate.mediaId === mediaId);
  if (!transcript) return [];

  const speakerNumbers = new Map<string, number>();
  const paragraphs: MutableParagraph[] = [];
  let current: MutableParagraph | null = null;

  transcript.words.forEach((word, index) => {
    const speakerId = word.speaker?.trim() || null;
    if (speakerId !== null && !speakerNumbers.has(speakerId)) {
      speakerNumbers.set(speakerId, speakerNumbers.size + 1);
    }
    const span: TranscriptWordSpan = {
      isPause: false,
      index,
      text: word.text,
      startSeconds: word.startSeconds,
      endSeconds: word.endSeconds,
      ...(typeof word.confidence === "number" && Number.isFinite(word.confidence)
        ? { confidence: word.confidence }
        : {}),
      lowConfidence:
        typeof word.confidence === "number" &&
        Number.isFinite(word.confidence) &&
        word.confidence < lowConfidenceThreshold,
    };

    if (current && (speakerId === null || speakerId === current.speakerId)) {
      const gap = roundMilliseconds(word.startSeconds - current.endSeconds);
      if (gap >= pauseChipMinSeconds) {
        current.spans.push({
          isPause: true,
          index,
          startSeconds: current.endSeconds,
          endSeconds: word.startSeconds,
          durationSeconds: gap,
        });
      }
      current.spans.push(span);
      current.endSeconds = Math.max(current.endSeconds, word.endSeconds);
      return;
    }

    current = {
      id: `paragraph-${index}`,
      speakerId,
      speakerNumber: speakerId === null ? null : (speakerNumbers.get(speakerId) ?? null),
      startSeconds: word.startSeconds,
      endSeconds: word.endSeconds,
      spans: [span],
    };
    paragraphs.push(current);
  });

  return paragraphs;
}

/** "N words to check" in the transcript footer. */
export function lowConfidenceWordCount(paragraphs: readonly TranscriptParagraph[]): number {
  return paragraphs.reduce(
    (count, paragraph) =>
      count + paragraph.spans.filter((span) => !span.isPause && span.lowConfidence).length,
    0,
  );
}

/**
 * The transcript word index to highlight at `playheadSeconds` (transcript source time). A word is
 * current from its start (inclusive) to its end (exclusive), and stays current through a short gap
 * before the next word of the same paragraph. Pauses, speaker changes, and time outside the words
 * have no current word.
 */
export function currentWordIndex(
  paragraphs: readonly TranscriptParagraph[],
  playheadSeconds: number,
): number | null {
  for (const paragraph of paragraphs) {
    if (playheadSeconds < paragraph.startSeconds || playheadSeconds >= paragraph.endSeconds) continue;
    let candidate: TranscriptWordSpan | null = null;
    for (const span of paragraph.spans) {
      if (span.startSeconds > playheadSeconds) break;
      candidate = span.isPause ? null : span;
      if (!span.isPause && playheadSeconds < span.endSeconds) return span.index;
    }
    const next = candidate
      ? paragraph.spans.find((span) => span.startSeconds > playheadSeconds)
      : undefined;
    if (candidate && next && !next.isPause) return candidate.index;
    return null;
  }
  return null;
}
