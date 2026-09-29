# Codex List Models Context

## Problem

Palmier's chat surface makes agent tool use visible: model listing, timeline reads, and generation actions appear as part of the conversation context. Video Creater's Codex rail already shows `get_timeline` and `project_context`, but it does not show generation model availability before an edit or asset request. That makes the assistant feel less connected to the media generation system even though the app already has mode-specific fal.ai model defaults.

## Goals

- Add a compact `list_models` context row to the Codex rail.
- Show image, video, and audio generation capability counts without adding backend calls.
- Keep existing `get_timeline` and `project_context` rows unchanged.
- Make the row accessible and testable as a tool context item.

## Non-goals

- No new model settings backend.
- No Temporal workflow changes.
- No fal.ai API call or credential handling.
- No changes to the media generation request payload.

## UX Requirements

- The context tools panel should include `list_models` above timeline and project context.
- The row should read as ready when default image/video/audio model families are available.
- The detail should be compact enough for the narrow Codex rail.
- The copy should avoid exposing credentials or implementation secrets.

## Verification

- Add AgentPanel coverage for the `list_models` row and detail text.
- Run the focused AgentPanel test file, then the full test and lint suite.
- Smoke-check the Codex rail in the browser at desktop width.
