# Real Local Media Preview Design

## Context

Palmier's editor keeps real footage visible in the central viewer while the media library,
timeline, chat, and source inspector stay available. The provided Palmier screenshots show
timeline and source tabs above a playback surface that displays the selected source media,
generated outputs, and timeline result. Palmier's docs also describe double-clicking source
footage, inspecting generated prompt/reference details, and iterating without leaving the
project.

Video Creater already has the surrounding editor structure: viewer tabs, source metadata,
right-rail source details, generated provenance, timeline transport chrome, and file-backed
project/media paths. The current preview surface is still deliberately synthetic. The
`source-aware-preview-surface` slice rendered source identity, kind, range, and path, but
explicitly excluded actual media decoding or file URL loading. The next gap is to make opened
local sources visibly previewable while keeping project files as the source of truth.

Reference requirements:

- Palmier docs: generate/import media into the library, double-click clips to open source
  footage, inspect generated prompt/frames/references, and iterate in context.
- Screenshot behavior: source tabs show real image/video frames, the transport remains below
  the viewer, and the right inspector remains focused on the selected source or timeline.

## Goal

Render actual local project media in `PreviewPanel` source mode for video, generated video,
image, and audio assets, using project-relative media paths resolved by the Tauri runtime.

## Non-Goals

- No timeline compositing or final rendered timeline playback in this slice.
- No frame-accurate scrubbing, waveform extraction, or thumbnail caching.
- No remote URL loading.
- No media mutation or canonical project file writes.
- No Temporal workflow changes. Preview is immediate editor display, not queued work.
- No use of provider API keys or generated-provider execution.

## Architecture

### Source URL Resolution

`EditorWorkspace` should enrich `PreviewSource` with an optional `previewUrl`.

- For embedded sample projects with no project directory, keep `previewUrl` null and retain
  the current synthetic source viewer treatment.
- For split projects, resolve `media.relativePath` against `projectDir`.
- Convert the absolute local path to a browser-safe Tauri asset URL at the UI boundary.
- Do not serialize the resolved absolute path or preview URL back into project files.

The URL resolution belongs in the frontend workspace layer because the preview is UI-only and
because `MediaAsset.relativePath` remains the durable text-file contract.

### Preview Rendering

`PreviewPanel` should keep its presentational boundary and render media based on the existing
`PreviewSource` fields plus `previewUrl`.

- `video` and `generated` sources render a muted `<video>` element with controls disabled by
  default and `playsInline`.
- `image` sources render an `<img>` with object-fit containment.
- `audio` sources render an `<audio>` element plus the existing waveform-style visual fallback.
- If loading fails or no `previewUrl` is available, show the existing synthetic treatment with
  the same metadata and path labels.

The existing transport remains visible. In this slice, its play button may toggle media playback
only when a media element is present; timeline playhead state stays unchanged.

### Safety And File Boundaries

- Only project-relative media paths from trusted project state are resolved.
- Paths must remain under `projectDir`; attempts to preview `..` escapes fall back to the
  synthetic treatment.
- The UI should not fetch remote URLs or arbitrary prompt-provided paths.
- Resolved preview URLs must not appear in project actions, render reports, generated asset
  records, or committed artifacts.

## UI Behavior

1. Opening a source tab for `input.mp4` shows the real video frame inside the preview well when
   the file exists.
2. Opening a generated output source shows the completed generated video or image when present.
3. Opening an audio source shows the audio element and the current waveform treatment.
4. The source metadata strip remains readable above the preview.
5. Missing files, unsupported media, or embedded sample projects degrade to the existing visual
   placeholder without breaking tabs, transport, or source inspector actions.

## Data Flow

1. Media selection, source reveal, and timeline double-click continue to call existing source-tab
   paths.
2. `selectedPreviewSource(project, mediaId, context)` derives the existing labels and adds
   `previewUrl` when a safe local file URL can be created.
3. `PreviewPanel` chooses the real media renderer only when `viewerMode === "source"` and
   `selectedSource.previewUrl` is present.
4. No project action is emitted for preview-only navigation or playback.

## Testing

- Unit test `selectedPreviewSource` or its helper resolves project-relative media under
  `projectDir` and rejects path traversal.
- `PreviewPanel` renders `<video>` for video/generated sources with `previewUrl`.
- `PreviewPanel` renders `<img>` for image sources with `previewUrl`.
- `PreviewPanel` renders `<audio>` for audio sources with `previewUrl`.
- `PreviewPanel` falls back to the existing synthetic viewer when `previewUrl` is missing.
- `EditorWorkspace` split-project test proves selected source tabs receive preview-capable
  sources without writing preview URLs into project actions.

## Browser QA

- Desktop: open a split project with real video, image, generated output, and audio assets.
  Verify the center viewer displays real media without overlapping the transport or right rail.
- Narrow width: verify the source metadata strip wraps or truncates cleanly and the media element
  stays contained in the preview well.
- Failure path: temporarily point one media item at a missing file and verify the synthetic
  fallback appears with metadata still visible.

## Acceptance Criteria

- Real local media appears in source mode when a safe project-relative file exists.
- The app falls back cleanly for missing, embedded, or unsafe paths.
- Preview URL state remains UI-only and is never persisted to project files.
- Existing source tabs, source inspector details, generated variation actions, media selection,
  and timeline editing tests continue to pass.
- Focused preview tests, full frontend tests, lint, browser QA, `git diff --check`, and the
  repository secret-fragment scan pass.
