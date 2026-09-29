# Transcript Index Drift Validation Design

## Context

Palmier's connected-agent workflow depends on agents seeing full project context before trimming,
splitting, reordering, or adjusting clips. Video Creater's EDL-first edit pipeline depends on
word-level transcript sidecars in `transcripts/<media-id>.json`, and split projects now write a
bounded `transcripts/index.json` catalog for agent discovery. The index intentionally omits
transcript text and word payloads, but it is the first file an agent will read to find transcript
coverage, repair counts, raw artifacts, and usable source time ranges. If it drifts after manual
edits, agent EDL decisions can be based on stale transcript metadata.

Palmier reference: https://www.palmier.io/docs

## Goal

Report stale `transcripts/index.json` entries during split-project validation while keeping
`transcripts/<media-id>.json` sidecars as the canonical transcript source.

## Behavior

- `load_split_project` continues to ignore `transcripts/index.json`.
- `validate_split_project` reads `transcripts/index.json` only when the file exists and is valid
  JSON.
- Invalid `transcripts/index.json` JSON is reported as a validation issue, not a hard validation
  error.
- Each index entry must match an existing transcript sidecar by `mediaId`.
- Each index entry's `transcriptId`, `mediaId`, `path`, `engine`, `rawArtifactPath`,
  `repairCount`, `segmentCount`, `wordCount`, `startSeconds`, and `endSeconds` must match the
  referenced sidecar-derived values.
- Each transcript sidecar must have a matching index entry when `transcripts/index.json` exists.
- Duplicate index `mediaId` values are reported.
- The index remains a safe discovery artifact and still must not duplicate transcript text, word
  text, repair payloads, or confidence metadata.

## Non-Goals

- No transcript schema change.
- No transcription worker, Temporal workflow, model, render, or UI behavior change.
- No runtime loading semantics change.
- No requirement that raw transcript artifact files physically exist in this slice.

## Verification

- Split project tests prove a stale transcript index `wordCount` is reported with a fix that tells
  the agent to regenerate `transcripts/index.json`.
- Split project tests prove missing sidecar transcripts in the index are reported.
- Existing split-project tests continue to prove valid transcript sidecars pass validation and that
  the index does not expose transcript text.
