# Transcript Index Split Project Design

## Context

Palmier's connected-agent workflow depends on the agent seeing full project context and being able
to trim, split, reorder, and adjust clips from real source material. Video Creater's EDL-first edit
pipeline depends on word-level transcripts, and split projects already store canonical transcript
sidecars as `transcripts/<media-id>.json`. The transcripts directory still lacks a stable catalog
entry point, so agents must scan every sidecar before they can tell which media has usable words,
repairs, raw artifacts, or time coverage.

Palmier reference: https://www.palmier.io/docs

## Goal

Write an agent-readable `transcripts/index.json` catalog whenever split projects are saved, without
changing runtime project semantics.

## Behavior

- `save_split_project` writes `transcripts/index.json`.
- The index includes `schemaVersion` and a `transcripts` array.
- Each entry includes `transcriptId`, `mediaId`, `path`, `engine`, `rawArtifactPath`, `repairCount`,
  `segmentCount`, `wordCount`, `startSeconds`, and `endSeconds`.
- Entries are sorted by `mediaId` for deterministic diffs.
- `startSeconds` and `endSeconds` are omitted when a transcript has no words or segments.
- `load_split_project` ignores `transcripts/index.json`; canonical transcripts remain the
  individual `transcripts/<media-id>.json` files.
- `validate_split_project` ignores `transcripts/index.json` as catalog metadata and continues to
  validate individual transcript files.

## Non-Goals

- No transcript schema change.
- No transcription worker, Temporal workflow, model, render, or UI behavior change.
- No duplication of transcript text, word text, or repair payloads in the index.

## Verification

- Split project tests prove `transcripts/index.json` is written with transcript metadata and the
  project-relative sidecar path.
- Split project tests prove loading and validating a project with `transcripts/index.json` does not
  create an empty transcript or report index metadata as an invalid transcript sidecar.
