# Selected Generated Clip Settings Context

## Context

Palmier's timeline-native AI editing flow lets editors inspect a generated clip's prompt, first and
last frames, references, resolution, duration, and aspect ratio before rerunning or tweaking it.
Video Creater now lets selected generated timeline clips rerun, replace, and edit prompt drafts in
the Codex rail, but that selected clip block only shows model, prompt, reference count, and
reference chips. Generation settings are available in project files and the Source Inspector but
are not visible in the selected timeline clip context.

Palmier reference: https://www.palmier.io/docs

## Goal

Expose generated clip settings in the selected timeline clip block so Codex-side timeline iteration
has the same compact source-of-truth context as the manual inspector.

## Behavior

- Add generated settings to `AgentSelectedTimelineClipContext` for selected timeline clips whose
  source resolves to a generated asset.
- Show compact chips in the selected generated clip group for:
  - resolution as `<width>x<height>` when both values are finite;
  - duration as seconds when finite;
  - fps as `<fps> fps` when finite;
  - aspect ratio when present.
- Keep existing model, generated asset id, reference count, prompt, prompt draft, and action buttons.
- Do not show settings chips for imported clips, audio-only clips, captions, or generated assets
  with no finite setting values.

## Non-Goals

- No editable generation settings in this block.
- No new project action, schema field, Temporal workflow, fal.ai provider behavior, or render change.
- No changes to media-bin or source-inspector generated source details.

## Verification

- AgentPanel test: selected generated timeline clips render settings chips from Codex context.
- EditorWorkspace test: settings from the generated asset are passed into the Codex selected
  timeline clip context and displayed in the rail.
- Run focused AgentPanel and EditorWorkspace tests, TypeScript checks, whitespace checks, and the
  secret-fragment scan.
