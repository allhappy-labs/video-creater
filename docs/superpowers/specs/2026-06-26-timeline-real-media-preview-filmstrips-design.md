# Timeline Real Media Preview Filmstrips Design

## Context

Palmier's timeline shows source identity directly inside clips: video clips carry visual thumbnails, generated clips keep AI identity, and audio clips show waveform context. Video Creater already renders deterministic filmstrip blocks for video clips and waveform peaks for audio clips, but the timeline does not use the real preview URLs that the media bin, viewer, and inspectors already receive.

## Requirements

- Video timeline clips should render a real media preview strip when the clip source has a safe local preview URL.
- Generated video clips should preserve their AI badge, generated styling, workflow status, and provenance while showing the real preview.
- If a preview URL is missing or fails to load, the existing deterministic filmstrip fallback must remain.
- Timeline clip accessible names must remain based on the clip label so existing keyboard and test behavior stays stable.

## Design

- Add an optional `mediaPreviewUrls` prop to `TimelineEditor`, keyed by media id.
- Resolve each video clip's source media id with the existing `sourceMediaId` helper.
- Pass the resolved preview URL into `VideoFilmstrip`.
- `VideoFilmstrip` renders a muted, metadata-preloaded video preview when available and not failed.
- On preview error, hide the failed preview and render the existing deterministic frame strip.
- `EditorWorkspace` passes `previewUrlsForMedia(projectDir, project.media)` into the timeline using the same source already used by media and inspector panels.

## Verification

- Add timeline editor tests for preview URL rendering and fallback after preview error.
- Add an editor workspace test proving split-project preview URLs reach timeline clips.
- Run the focused timeline and editor workspace tests.
- Run `pnpm lint`.
- Browser-smoke the sample editor timeline and check video clips retain labels, AI badges, and no layout overlap.
