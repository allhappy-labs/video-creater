# Codex Timeline Reference Thumbnails Design

## Problem

Palmier makes AI-generated timeline clips inspectable in place: when a generated clip is selected,
the editor can see the prompt, model, first frame, last frame, and references without leaving the
timeline. Video Creater's Codex selected-source block now renders reference thumbnails, and
`EditorWorkspace` already passes preview URLs for generated timeline clip references. The selected
timeline clip context still renders those references as text-only chips, so the agent rail is
visually inconsistent and weaker for timeline-native AI editing.

## Goals

- Render selected generated timeline clip references as compact thumbnail rows in the Codex rail.
- Reuse `AgentSelectedReferenceContext.previewUrl` and the existing safe preview URL plumbing.
- Preserve the existing open-reference and mention-reference actions.
- Keep the fallback icon and text metadata for missing or unsafe previews.

## Non-Goals

- No new project schema fields.
- No changes to generated asset recording, replacement, variation, or Temporal workflow behavior.
- No changes to Source Inspector or MediaBin reference rendering.

## Design

`AgentPanel` will render `selectedTimelineClipContext.references` with the same visual treatment as
selected generated source references: a 16:9 thumbnail area, role label, filename, and `@mediaId`.
Image references use `img`, video/generated references use muted `video`, and missing previews use
a media-kind fallback icon.

The existing `Open <role> <mediaId>` button continues to call `onOpenSelectedTimelineReference`.
The separate `Mention <role> <mediaId>` button continues inserting the `@` mention into the Codex
prompt.

## Acceptance Criteria

- A selected generated timeline clip reference with a preview URL renders a thumbnail with an
  accessible label.
- The reference card still opens the referenced source footage.
- The `@` affordance still inserts the referenced media mention.
- Missing preview URLs still show the role, filename, media id, and fallback icon.
