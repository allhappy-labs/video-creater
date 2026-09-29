# Workflow Job Recency Order Design

Palmier keeps generation and edit activity close to the media and timeline workflow: recent jobs must be easy to inspect, rerun, or adjust without leaving the project. Video Creater already records workflow-backed jobs in text-editable project files and renders the latest jobs in the timeline inspector and Codex rail. The missing behavior is deterministic recency ordering when a human or agent edits project JSON out of append order.

## Goal

Show the most recently updated workflow jobs first anywhere the editor renders a compact recent-job list.

## Requirements

- Order recent workflow jobs by `updatedAt` descending instead of array position.
- Preserve stable ordering for jobs with the same timestamp.
- Place jobs with missing or invalid timestamps after valid timestamps.
- Use the same ordering in the Project Timeline Inspector and Codex workflow activity rail.
- Keep the visible list capped to the three most recent jobs.

## Non-Goals

- No project schema migration or job persistence rewrite.
- No Temporal worker/client/runtime changes.
- No workflow status polling or provider integration changes.
- No visual redesign beyond correcting the displayed ordering.

## Testing

- Unit coverage proves the shared helper sorts valid timestamps descending, preserves same-timestamp order, and sinks invalid timestamps.
- Project Timeline Inspector coverage proves the visible workflow queue is timestamp-ordered, not array-ordered.
- Agent Panel coverage proves Codex workflow activity uses the same newest-first list.
