# Palmier Inspector Viewer Action Design

## Intent

Palmier keeps the right rail focused on the active inspector. Video Creater still shows a separate `Timeline context` card above the source inspector, which duplicates project context and makes the rail feel more like a dashboard than an editor inspector. Move the source/timeline viewer action into the compact inspector context row and remove the duplicate context card whenever a source inspector is active.

## Requirements

- When a selected source clip or library media opens the source inspector, the right rail must not render the separate `Timeline context` region.
- The compact `Inspector context` row must still identify the active inspector view as `Source` or `Timeline`.
- If a source viewer is available, the context row must expose one compact viewer-switch button:
  - `Show timeline viewer` when the viewer is currently showing source footage.
  - `Show source viewer` when the viewer is currently showing the timeline.
- Clicking the compact viewer action must preserve the existing viewer switching behavior.
- The standalone `Timeline context` region must continue to render for timeline-only selections where no source inspector exists.
- No media selection, source inspector, timeline inspector, or Codex context data should change.

## Verification

- Update `EditorWorkspace` tests to prove the source-inspector right rail no longer contains `Timeline context`.
- Update viewer-switching coverage to use the compact inspector context action.
- Keep timeline-only inspector coverage for the standalone timeline context.
- Browser-smoke the editor to confirm the right rail has one focused inspector header and no duplicate context card for source selections.
