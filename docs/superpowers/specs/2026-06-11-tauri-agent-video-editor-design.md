# Tauri Agent-Assisted Video Editor Design

## Summary

Build a local-first Tauri desktop app for agent-assisted video editing. The app integrates HyperFrames, Codex, and dnd-timeline through a unified project timeline rather than letting any upstream tool own project state.

The selected architecture is Rust-orchestrated modular sidecars:

- React TypeScript frontend with shadcn/ui components for the app UI.
- dnd-timeline as the headless timeline interaction layer.
- Rust backend as the authority for project state, media jobs, render orchestration, Codex integration, validation, and storage.
- Codex integrated through a managed local `codex app-server` child process.
- HyperFrames integrated through a narrow local Node sidecar for full-frame scene and overlay asset rendering.
- ffmpeg and ffprobe controlled by Rust for media probing and final video rendering.

The MVP produces a complete edited video from raw footage: clips, captions, HyperFrames scenes, overlays, audio processing, and final `mp4`.

## Goals

- Provide a usable local desktop editor as the first screen, not a landing page.
- Let Codex assist with rough cuts, timeline changes, captions, overlays, render troubleshooting, and HyperFrames scene briefs.
- Keep the unified timeline as the single source of truth.
- Port the video-use-style render/transcription/EDL pipeline into Rust; no video-use compatibility layer is required.
- Support HyperFrames both as full-frame timeline scenes and as overlay/motion assets.
- Keep Rust in control of execution, validation, logs, cancellation, progress, and persistence.

## Non-Goals

- Hosted collaboration.
- Cloud rendering or render farms.
- Multi-user authentication.
- Mobile support.
- video-use file or API compatibility.
- Porting HyperFrames itself to Rust.
- Plugin marketplace or public extension system.

## Architecture

The app is a local-first Tauri desktop application.

### Frontend

The frontend is React TypeScript. Visible application controls and layout primitives use shadcn/ui components only. Custom rendering is allowed for the timeline canvas, clip bodies, waveform regions, media preview, and other editor-specific surfaces where shadcn/ui does not provide a primitive.

dnd-timeline provides headless drag, resize, and row positioning behavior. The app owns the visual track rows, clips, handles, labels, selection states, context menus, and inspector panels.

### Rust Backend

The Rust backend owns:

- Canonical project file loading, validation, mutation, and saving.
- Media import and path management.
- ffprobe media metadata extraction.
- Transcription job orchestration and transcript storage.
- Timeline patch validation and application.
- Render-plan generation.
- ffmpeg command construction and execution.
- HyperFrames sidecar supervision and render requests.
- Codex app-server process lifecycle and JSON-RPC communication.
- Progress, cancellation, logs, retries, and artifact metadata.

### Sidecars

Codex runs through `codex app-server` as a managed local child process. The backend connects to it over a supported app-server transport, starts or resumes a project-linked thread, and streams events into the UI.

HyperFrames runs through a narrow Node worker process. Rust sends scene render requests containing duration, dimensions, fps, scene inputs, and output paths. The worker returns generated asset metadata. HyperFrames does not own project state.

ffmpeg and ffprobe are treated as external binaries controlled by Rust.

## Timeline Model

The unified project timeline is the single source of truth. It stores tracks, clips, time ranges, media references, generated scenes, overlays, captions, audio state, and render settings.

Track types:

- `video`: primary raw footage or rendered full-frame assets.
- `hyperframe_scene`: full-frame HyperFrames scenes that occupy timeline time like clips.
- `overlay`: transparent or composited assets, including HyperFrames overlays.
- `caption`: generated and editable subtitle cues.
- `audio`: source audio, music, voiceover, gain, and fade automation.

dnd-timeline edits this model through adapter hooks:

- Timeline rows map to tracks.
- Timeline items map to clips, scenes, overlays, captions, or audio regions.
- Drag and resize actions become typed timeline patches.
- Patch validation happens in Rust before the canonical project is mutated.

HyperFrames internal timing is scoped inside a HyperFrames scene or overlay. The unified timeline provides the scene duration, dimensions, fps, input variables, start time, and clipping window.

## Agent Assistance

Codex is integrated through app-server, not one-shot `codex exec` jobs. The app keeps a project-linked Codex thread so editing context persists across turns.

Codex does not directly mutate the canonical project file. Rust sends bounded context to Codex:

- Project summary.
- Media metadata.
- Transcript excerpts.
- Current timeline slice or selected range.
- Render or validation logs when relevant.
- The user's edit request.

Codex returns structured proposals:

- Timeline patches.
- Rough-cut plans.
- Caption edits.
- HyperFrames scene briefs.
- Overlay suggestions.
- Render troubleshooting notes.

Rust validates proposals and the UI presents them for preview, accept, reject, or apply-to-selection actions.

MVP agent workflows:

- Generate an initial rough cut from transcripts and media metadata.
- Suggest cuts, pacing changes, overlays, captions, and HyperFrames scenes.
- Convert natural-language instructions into timeline patches.
- Explain render or validation failures using relevant logs.
- Iterate on selected clips or timeline ranges.

## Rendering Pipeline

Rust ports the video-use-style pipeline natively. video-use is a reference for workflow shape, not a dependency or compatibility target.

MVP render stages:

1. Probe imported media with ffprobe and store stable metadata.
2. Transcribe audio into word-level transcript data.
3. Build a render plan from the canonical timeline.
4. Render HyperFrames full-frame scenes and overlay assets through the Node sidecar.
5. Extract, trim, and grade raw clips with ffmpeg.
6. Concatenate or compose primary video layers.
7. Composite overlays and HyperFrames assets.
8. Burn or mux captions depending on export settings.
9. Normalize loudness and export final `mp4`.

The backend owns render progress, cancellation, log capture, retry behavior, artifact paths, and validation. The HyperFrames sidecar only receives scene render requests and returns artifact metadata.

Transcription is pluggable. The required transcript shape is:

- Source media id.
- Word text.
- Start time.
- End time.
- Confidence, when available.
- Speaker or channel, when available.

## Frontend Experience

Primary MVP screens:

- Project workspace with media bin, preview player, inspector, timeline, and Codex agent panel.
- Import and transcription status with queued jobs, media metadata, and transcript readiness.
- Agent suggestions with proposed timeline patches and accept/reject/apply-to-selection controls.
- Render queue with draft/final export status, logs, and artifact links.

The first usable screen is the editor workspace. It should support import, rough-cut generation, timeline correction, preview, and final render without leaving the app.

## Data And Storage

Projects are local-first and file-backed.

```text
project/
  video-creater.project.json
  media/
  transcripts/
  generated/
    hyperframes/
    previews/
  renders/
  logs/
```

The canonical project JSON stores:

- Stable project id.
- Relative paths to media and generated artifacts.
- Media metadata.
- Transcript references.
- Timeline tracks and items.
- Render settings.
- Codex thread id.
- Job history summaries.

Large generated artifacts stay as files in the project folder and are referenced from the JSON by stable id and relative path.

Secrets do not live in project files. Codex uses the user's existing local Codex authentication and configuration. Any transcription provider keys are read from local app settings or environment variables.

## Validation And Error Handling

Rust validates project files on load and before every render.

Timeline patches must be typed and validated before application. Validation should reject:

- Negative durations.
- Missing media or artifact references.
- Track type mismatches.
- Unsupported overlap rules for a track type.
- Render settings incompatible with a target export.

Jobs produce structured status events: queued, running, progress, blocked, failed, cancelled, and completed. Failures include a user-facing summary plus a log reference. Codex can use those logs for troubleshooting suggestions, but Rust remains responsible for the actual retry or state mutation.

## Testing Strategy

Rust tests:

- Project schema validation.
- Timeline patch validation and application.
- Render-plan generation.
- ffmpeg command construction.
- Job state transitions.
- HyperFrames sidecar request/response parsing.
- Codex app-server event parsing.

TypeScript tests:

- Timeline adapter mapping between dnd-timeline items and canonical patches.
- Patch preview and apply behavior.
- Agent suggestion accept/reject flows.

Integration tests:

- Tiny sample media fixture that renders a short playable `mp4`.
- HyperFrames sidecar smoke render.
- Basic app bridge commands through Tauri.

UI verification:

- Browser/Playwright checks for core timeline interactions where practical.
- Visual checks for editor layout, timeline selection, resize handles, and logs panel.

## Milestones

1. Tauri shell with React TypeScript, shadcn/ui, Rust command/event bridge, and project create/open/save.
2. Canonical timeline schema, validation, and patch application.
3. dnd-timeline editor bound to the schema with imported media items.
4. Rust media probing and render-plan generation.
5. Native Rust ffmpeg render path for basic cuts and final `mp4`.
6. HyperFrames Node sidecar for full-frame scenes and overlays.
7. Codex app-server integration for structured timeline patch suggestions.
8. Transcription import/generation and rough-cut workflow.
9. End-to-end draft/final render UX.

## Open Implementation Decisions

These are implementation choices, not product scope blockers:

- Exact transcript engine and credential storage mechanism.
- Exact project JSON schema versioning format.
- Whether HyperFrames worker communicates with Rust over stdio JSONL or a localhost transport.
- Whether previews use proxy media, reduced resolution renders, or timeline-only approximations first.
- Exact shadcn/ui theme tokens.
