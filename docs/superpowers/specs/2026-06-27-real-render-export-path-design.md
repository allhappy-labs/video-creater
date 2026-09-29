# Real Render Export Path Design

## Goal

Replace the UI WebM export sample report and Temporal render placeholders with a real Rust-owned render path that loads a split project, builds an EDL-backed render plan from timeline source ranges, renders through the existing GStreamer/GES backend, validates output streams, writes render-review artifacts, and attaches durable project actions.

## Current Gap

The editor WebM export flow calls `buildSampleRenderReport`, records a completed render job, and attaches a derived report without running Rust render code. Temporal worker registrations also expose `BuildRenderPlan`, `RenderMedia`, `ValidateRenderedMedia`, `AttachRenderReport`, `ValidateExportProfile`, `WriteExportArtifact`, and `AttachExportReport`, but most return generic registered placeholders for render/export-media inputs.

## Architecture

Add a reusable `render_pipeline::project_export` module. It owns split-project WebM rendering: load the project, select the primary video media and ordered video clips with `sourceIn`/`sourceOut`, build a `RenderPlan`, invoke `GstreamerGesRenderBackend`, probe the output, write JSON/Markdown reports, convert the pipeline report into `ProjectRenderReport`, and apply `recordJob`, `updateJobStatus`, and `attachRenderReport` actions.

Expose a Tauri command `render_webm_to_split_project_folder` for the UI draft/final WebM buttons. Reuse the same module from Temporal activities so worker paths execute real build/render/validate/attach behavior when given export-media or render-draft runtime inputs.

## Data Flow

1. UI calls `render_webm_to_split_project_folder(projectDir, profile, jobId, updatedAt)`.
2. Rust loads the split project and builds an output under `renders/<jobId>/output.webm`.
3. Rust renders selected source ranges through GStreamer/GES and writes `renders/<jobId>/report.json`, `report.md`, and `render.log`.
4. Rust attaches a `ProjectRenderReport` with duration, stream checks, artifact paths, and log path.
5. UI uses the returned project and render report instead of a sample report.
6. Temporal render/export-media activities call the same implementation helpers.

## Error Handling

The command fails before mutation when the split folder is missing, no primary video clip exists, `sourceIn`/`sourceOut` are invalid, media paths are missing, the backend fails, output validation fails, or report paths cannot be written. Job status is only marked completed after the output is rendered, probed, and the report action applies.

## Testing

Rust tests cover render-plan construction from split-project timeline clips, render report conversion, command-level action output with a fake backend, and Temporal activity values for build/render/validate/attach. Frontend tests verify WebM export invokes the Rust command and no longer calls `attachRenderReport` using sample data.

## Self-Review

- No placeholders: all outputs and paths are concrete.
- Scope is one subsystem: real WebM render/export/report path, plus Temporal activity reuse.
- Ambiguity resolved: MP4 profile export remains policy-gated; this feature makes WebM UI export and Temporal render activity boundaries real first.
