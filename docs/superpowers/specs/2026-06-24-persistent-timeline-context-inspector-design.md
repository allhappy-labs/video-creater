# Persistent Timeline Context Inspector

## Context

Palmier keeps project and timeline facts visible in the right inspector while editors work on media and timeline clips. Video Creater has a full project timeline inspector, but selecting a source clip or library media replaces it entirely with source details. That makes project duration, render format, track count, and workflow activity disappear at the moment an editor is making clip-level decisions.

## Goal

Keep compact timeline context visible in the right rail whenever a source clip or library media source is selected, without duplicating the full workflow-heavy project inspector.

## Behavior

- Add a compact `Timeline context` region above selected source/media details in the inspector rail.
- Show project name, project storage mode, duration, render format, timeline contents, and active workflow count.
- Show the active viewer target as `Timeline` or `Source: <label>` so the right rail explains what the center preview is showing.
- When a source viewer tab is active, expose a compact `Show timeline viewer` action in the context card.
- Clicking `Show timeline viewer` switches the center preview back to the timeline tab without clearing the selected source/media inspector.
- When the timeline viewer tab is active but a source/media inspector remains selected, expose `Show source viewer`.
- Clicking `Show source viewer` returns the center preview to the active selected source tab without clearing the right rail.
- Keep the existing full `Project timeline inspector` when no source/media inspector is active.
- Keep source clip, selected media, generated output, replacement, and queue actions unchanged.
- Do not show the compact context twice when the full project inspector is visible.

## Non-Goals

- No new project file schema.
- No timeline mutation behavior.
- No Temporal workflow contract changes.
- No provider or generation request changes.

## Verification

- Editor workspace tests prove selected library media still shows `Source Inspector` and now also shows `Timeline context`.
- Editor workspace tests prove selected timeline source clips show `Timeline context` above the source inspector controls.
- Editor workspace tests prove the context card labels the active source viewer and timeline viewer states.
- Editor workspace tests prove `Show timeline viewer` returns the preview to the timeline tab while the right rail keeps the selected source controls.
- Editor workspace tests prove `Show source viewer` returns the preview to the selected source tab after the timeline viewer is shown.
- Existing project timeline inspector tests continue to cover the full inspector, workflow queue, exports, render reports, and generated media summaries.
