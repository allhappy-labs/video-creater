import { describe, expect, it } from "vitest";

import type { Transcript, VideoProject } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";

import {
  currentWordIndex,
  lowConfidenceWordCount,
  transcriptParagraphs,
  type TranscriptParagraph,
} from "./transcript-view-model";

type Word = Transcript["words"][number];

function word(text: string, startSeconds: number, endSeconds: number, extra: Partial<Word> = {}): Word {
  return { text, startSeconds, endSeconds, ...extra };
}

function projectWithWords(words: Word[], mediaId = "media-1"): VideoProject {
  const project = fixtureProject();
  project.transcripts = [{ id: "transcript-test", mediaId, repairs: [], segments: [], words }];
  return project;
}

function spanTexts(paragraph: TranscriptParagraph) {
  return paragraph.spans.map((span) => (span.isPause ? `[${span.durationSeconds}s]` : span.text));
}

describe("transcriptParagraphs", () => {
  it("returns no paragraphs when the media has no transcript", () => {
    expect(transcriptParagraphs(fixtureProject(), "media-voiceover")).toEqual([]);
  });

  it("splits paragraphs on speaker change and numbers speakers by first appearance", () => {
    const project = projectWithWords([
      word("Hi", 0, 0.3, { speaker: "spk_b" }),
      word("there", 0.35, 0.7, { speaker: "spk_b" }),
      word("Hello", 0.8, 1.2, { speaker: "spk_a" }),
      word("again", 1.25, 1.6, { speaker: "spk_b" }),
    ]);

    const paragraphs = transcriptParagraphs(project, "media-1");

    expect(paragraphs.map((paragraph) => [paragraph.speakerId, paragraph.speakerNumber, spanTexts(paragraph)])).toEqual([
      ["spk_b", 1, ["Hi", "there"]],
      ["spk_a", 2, ["Hello"]],
      ["spk_b", 1, ["again"]],
    ]);
    expect(paragraphs.map((paragraph) => [paragraph.startSeconds, paragraph.endSeconds])).toEqual([
      [0, 0.7],
      [0.8, 1.2],
      [1.25, 1.6],
    ]);
  });

  it("keeps words without a speaker label in the current speaker's paragraph", () => {
    const project = projectWithWords([
      word("So", 0, 0.2, { speaker: "a" }),
      word("um", 0.25, 0.4, { speaker: null }),
      word("yes", 0.45, 0.7, { speaker: "a" }),
    ]);

    const paragraphs = transcriptParagraphs(project, "media-1");

    expect(paragraphs.map(spanTexts)).toEqual([["So", "um", "yes"]]);
    expect(paragraphs[0]?.id).toBe("paragraph-0");
  });

  it("keeps a single unlabelled paragraph when words carry no speaker", () => {
    const paragraphs = transcriptParagraphs(fixtureProject(), "media-1");

    expect(paragraphs).toHaveLength(1);
    expect(paragraphs[0]?.speakerId).toBeNull();
    expect(paragraphs[0]?.speakerNumber).toBeNull();
    expect(paragraphs[0] && spanTexts(paragraphs[0])).toEqual(["Original", "caption", "Second", "split"]);
  });

  it("carries the transcript word index on every word span", () => {
    const project = projectWithWords([
      word("One", 0, 0.2, { speaker: "a" }),
      word("two", 0.3, 0.5, { speaker: "b" }),
      word("three", 0.6, 0.9, { speaker: "b" }),
    ]);

    const indices = transcriptParagraphs(project, "media-1").flatMap((paragraph) =>
      paragraph.spans.flatMap((span) => (span.isPause ? [] : [span.index])),
    );

    expect(indices).toEqual([0, 1, 2]);
  });

  it("inserts a pause chip for gaps of 0.6 seconds or more, and not for shorter gaps", () => {
    const project = projectWithWords([
      word("Short", 0.45, 1.05),
      word("gap", 1.64, 2),
      word("exact", 2.6, 3),
      word("long", 4.25, 4.5),
    ]);

    const [paragraph] = transcriptParagraphs(project, "media-1");

    expect(paragraph && spanTexts(paragraph)).toEqual(["Short", "gap", "[0.6s]", "exact", "[1.25s]", "long"]);
    expect(paragraph?.spans[2]).toEqual({
      isPause: true,
      index: 2,
      startSeconds: 2,
      endSeconds: 2.6,
      durationSeconds: 0.6,
    });
  });

  it("does not add a pause chip across a speaker change", () => {
    const project = projectWithWords([
      word("Question", 0, 0.5, { speaker: "a" }),
      word("Answer", 2, 2.5, { speaker: "b" }),
    ]);

    const paragraphs = transcriptParagraphs(project, "media-1");

    expect(paragraphs.map(spanTexts)).toEqual([["Question"], ["Answer"]]);
  });

  it("marks words below 0.6 confidence only when confidence is present", () => {
    const project = projectWithWords([
      word("sure", 0, 0.2, { confidence: 0.6 }),
      word("maybe", 0.25, 0.5, { confidence: 0.59 }),
      word("unknown", 0.55, 0.8, { confidence: null }),
      word("missing", 0.85, 1),
    ]);

    const [paragraph] = transcriptParagraphs(project, "media-1");
    const words = paragraph?.spans.flatMap((span) => (span.isPause ? [] : [span])) ?? [];

    expect(words.map((span) => [span.text, span.confidence, span.lowConfidence])).toEqual([
      ["sure", 0.6, false],
      ["maybe", 0.59, true],
      ["unknown", undefined, false],
      ["missing", undefined, false],
    ]);
    expect(lowConfidenceWordCount(paragraph ? [paragraph] : [])).toBe(1);
  });
});

describe("currentWordIndex", () => {
  const project = projectWithWords([
    word("First", 1, 1.4, { speaker: "a" }),
    word("second", 1.5, 2, { speaker: "a" }),
    word("after", 3, 3.5, { speaker: "a" }),
    word("reply", 3.6, 4, { speaker: "b" }),
  ]);
  const paragraphs = transcriptParagraphs(project, "media-1");

  it("is null before the first word", () => {
    expect(currentWordIndex(paragraphs, 0)).toBeNull();
    expect(currentWordIndex(paragraphs, 0.999)).toBeNull();
  });

  it("includes a word's start and excludes its end", () => {
    expect(currentWordIndex(paragraphs, 1)).toBe(0);
    expect(currentWordIndex(paragraphs, 1.399)).toBe(0);
    expect(currentWordIndex(paragraphs, 1.5)).toBe(1);
  });

  it("holds the previous word through a short gap", () => {
    expect(currentWordIndex(paragraphs, 1.45)).toBe(0);
  });

  it("is null during a pause chip and across a speaker change", () => {
    expect(currentWordIndex(paragraphs, 2)).toBeNull();
    expect(currentWordIndex(paragraphs, 2.9)).toBeNull();
    expect(currentWordIndex(paragraphs, 3.55)).toBeNull();
  });

  it("is null at and after the last word end", () => {
    expect(currentWordIndex(paragraphs, 3.99)).toBe(3);
    expect(currentWordIndex(paragraphs, 4)).toBeNull();
    expect(currentWordIndex([], 1)).toBeNull();
  });
});
