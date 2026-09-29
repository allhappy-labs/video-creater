# Temporal Workflow Contract Design

## Context

Video Creater already tracks `jobs`, generated asset statuses, transcripts, render reports, and project actions. Those records are not yet tied to Temporal workflow executions, task queues, workflow types, or activity types. The active product direction requires queues, scheduling, and workflows to use Temporal Rust.

Temporal Rust documentation shows the current SDK model: workers poll a named task queue, register the exact workflow and activity types they can execute, and workflow clients start executions against those task queues. This first slice creates the project-level contract needed before migrating render, generation, transcription, and Codex-edit execution into real Temporal workers.

## Goals

- Add a Rust workflow contract module with stable Temporal task queue, workflow type, and activity type names for Video Creater jobs.
- Provide deterministic workflow IDs for project-scoped jobs.
- Extend `JobSummary` with optional Temporal workflow metadata while keeping existing project JSON backward compatible.
- Provide a helper that creates a `JobSummary` with Temporal metadata for new orchestration paths.
- Keep this slice independent from a running Temporal server.

## Non-Goals

- No Temporal worker process is started in this slice.
- No `temporalio-*` crate dependency is added yet.
- No render, transcription, media generation, or Codex edit path is migrated to a worker yet.
- No visual frontend UI changes.

## Workflow Contract

All new workflow-backed job records use task queue `video-creater-workflows`.

Workflow kinds:

- `generate_media`: workflow type `VideoCreaterGenerateMediaWorkflow`.
- `render_draft`: workflow type `VideoCreaterRenderDraftWorkflow`.
- `transcribe_media`: workflow type `VideoCreaterTranscribeMediaWorkflow`.
- `codex_edit`: workflow type `VideoCreaterCodexEditWorkflow`.

Activity sets:

- Generate media: record prompt, call provider, import output, attach generated asset result.
- Render draft: build render plan, render media, validate output, attach render report.
- Transcribe media: probe media, run transcription, store transcript.
- Codex edit: collect context, request proposal, validate project actions, persist accepted proposal state.

Workflow IDs are deterministic and project scoped:

```text
video-creater/<project-id>/<job-kind>/<job-id>
```

Each segment is lowercased and sanitized so the ID is stable for local project files and Temporal lookups.

## Data Model

`JobSummary` gains:

```rust
pub workflow: Option<TemporalWorkflowMetadata>
```

The field is skipped when absent so existing projects keep the same JSON shape. When present it serializes as:

```json
{
  "workflow": {
    "workflowId": "video-creater/project-1/render-draft/job-1",
    "workflowType": "VideoCreaterRenderDraftWorkflow",
    "taskQueue": "video-creater-workflows",
    "runId": null,
    "activityTypes": ["BuildRenderPlan", "RenderMedia", "ValidateRenderedMedia", "AttachRenderReport"]
  }
}
```

## Frontend Command Boundary

The React editor must not duplicate workflow task queue names, workflow type names, activity type lists, or workflow ID sanitization. New UI paths that need workflow-backed job records call the Tauri command:

```text
build_temporal_job_summary(kind, projectId, jobId, status, updatedAt)
```

The command delegates to the Rust workflow contract module and returns the same `JobSummary` shape used by project actions. This keeps text-file project actions ready for the future Temporal Rust client/worker integration while the manual editor can still queue local sample render and media-generation records without a running Temporal service.

Remaining live-worker gap:

- Add Temporal Rust SDK dependencies after the command contract is stable.
- Start a worker on `video-creater-workflows`.
- Register workflow and activity implementations matching the Rust contract names.
- Replace local sample render/media-generation completion with workflow client start and status polling.

## Acceptance Criteria

- Old job summaries deserialize without workflow metadata.
- New workflow-backed job summaries serialize with `workflowId`, `workflowType`, `taskQueue`, `runId`, and `activityTypes`.
- Workflow IDs are deterministic and sanitize unsafe project/job strings.
- Frontend render and generated-media queue paths ask Rust to build workflow-backed `JobSummary` records.
- All new contract APIs compile without a running Temporal service.
- Focused Rust tests and a full frontend test pass before commit.
