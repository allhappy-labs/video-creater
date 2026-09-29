# Codex Selected Reference Thumbnails Design

## Problem

Palmier keeps generated media provenance visible while the editor stays in context: first frame,
last frame, and reference media appear as visual cards in source/detail panels. Video Creater's
Source Inspector already does this with local preview thumbnails, but the Codex selected-source
block still renders references as text-only chips. Agents and editors can see that references
exist, but they cannot visually confirm the source frames from the chat rail.

## Goals

- Show compact thumbnails for generated source references in the Codex selected-source block.
- Reuse the existing safe local preview URL conversion from `EditorWorkspace`.
- Preserve the current open-reference and mention-reference actions.
- Keep references useful when no preview URL exists by showing the current text metadata and a
  media-kind fallback icon.

## Non-Goals

- No new project schema fields.
- No changes to generated asset recording, Temporal workflow inputs, or fal.ai provider behavior.
- No media loading in `AgentPanel`; it receives already-sanitized preview URLs from workspace
  context.

## Design

`AgentSelectedReferenceContext` gains an optional `previewUrl`. `EditorWorkspace` fills it while
building selected generated media and selected generated timeline clip contexts by resolving the
reference media through `previewUrlForMedia(projectDir, relativePath)`. Unsafe paths continue to
resolve to `null`.

`AgentPanel` renders each selected source reference as a compact thumbnail row: image references use
`img`, video/generated references use muted `video`, and missing previews fall back to a media-kind
icon. The existing button still opens the reference, while the secondary `@` button still inserts a
mention.

## Acceptance Criteria

- A Codex-selected generated source reference with a preview URL renders a thumbnail with an
  accessible label.
- Clicking the reference thumbnail card still opens the referenced source.
- Clicking the `@` affordance still inserts that reference as a mention.
- Unsafe or missing preview URLs keep the existing text-only provenance usable with a fallback
  icon.
