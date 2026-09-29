# Temporal Workflow Start Pending State Design

## Context

Palmier-style AI editing treats generated media and export work as timeline-native operations that can be started from the editor and then monitored. Video Creater now records Temporal start requests and can dispatch them through the Tauri command, but the workflow queue button remains clickable while that async start request is in flight. Live Temporal uses `rejectDuplicate`, yet the manual editor should not send duplicate start attempts for the same queued job.

## Goal

Make queued Temporal workflow starts single-dispatch from the editor UI by tracking pending start requests per job id.

## Requirements

- When a queued workflow start is in flight, the corresponding Workflow queue action is disabled.
- The button label changes to `Starting...` so the editor can distinguish a pending dispatch from setup gating.
- A rapid second click for the same job must not call `start_temporal_workflow` again.
- Pending state clears after either a successful `started` result, an unavailable/runtime error result, or a thrown command error.
- Existing setup gating remains unchanged: missing Temporal preflight still shows `Temporal setup needed`.
- No project schema, Temporal workflow payload, fal.ai provider behavior, or Rust command changes in this slice.

## Acceptance Tests

- `ProjectTimelineInspector` renders a pending queued job with a disabled `Starting...` action and does not call `onStartWorkflow`.
- `EditorWorkspace` ignores duplicate start clicks while the first command is unresolved.
- Existing unavailable-runtime handling still reports the returned message after the pending start clears.
