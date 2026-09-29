# Palmier Codex Workflow Activity Compaction Design

## Goal

Make Codex workflow activity read like a compact editor status feed instead of a Temporal diagnostics panel. The chat rail should show what is happening and why it matters, while workflow IDs, task queues, activity counts, credential environment names, and start-request policies stay in project files, logs, and dedicated workflow diagnostics.

## Design

- Keep the `Codex workflow activity` region and active workflow count.
- Keep one compact row per recent workflow job with job kind, user-facing status, model, prompt, references, and generation settings when available.
- Remove visible task queue names, workflow IDs, start-request readiness/policy/activity-count text, and provider credential labels from Codex workflow activity rows.
- Preserve workflow ordering, the three-job cap, transcript tool rows, project workflow queue behavior, Temporal job data, and generated asset/job schemas.

## Testing

- `AgentPanel` verifies Codex workflow activity still shows active count, job kind, status, model, prompt, references, and settings.
- `AgentPanel` verifies Codex workflow activity does not render task queues, workflow IDs, start-request policy/activity-count text, or credential environment names.
- Existing workflow ordering coverage remains unchanged.
