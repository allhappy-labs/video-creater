# Codex Insert Runtime Gate Design

## Context

Palmier chat can place existing media on the timeline as a project edit. Video Creater routes
explicit `@media` placement prompts to timeline insertion, but the primary composer action is still
rendered only inside the EDL generation-ready branch. If the local transcription model or runtime is
not ready, the composer shows only `Open model settings` even though timeline insertion does not
need transcription.

## Goal

Keep insertion-only Codex actions available when EDL generation is blocked by transcription or
runtime readiness.

## Behavior

- A resolved insertable mention with explicit placement intent shows `Insert mention on timeline`
  as the primary composer action even when `transcriptionModelReady` or `runtimeReady` is false.
- Pressing that action continues to call the existing validated insertion callback and records the
  same `project_action` transcript entry.
- Ordinary EDL prompts still require `hasExistingTranscript` or ready transcription/runtime.
- When generation is blocked and no insertion intent is available, the composer keeps the existing
  warning and `Open model settings` button.

## Non-Goals

- No change to transcription requirements for `Generate edit`.
- No new project action schema or direct project-file mutation.
- No automatic insertion without a clear placement prompt.
- No broader natural-language parser.

## Verification

- `AgentPanel` tests cover an explicit placement prompt with transcription unavailable.
- Existing blocked generation tests continue to pass.
- Existing workspace mention insertion tests continue to prove the Rust-validated project-action
  path is used.
