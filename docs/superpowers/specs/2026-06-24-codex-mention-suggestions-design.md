# Codex Mention Suggestions Design

## Context

Palmier chat lets editors use `@` to reference specific project media while asking the assistant to generate or edit in context. Video Creater now resolves typed `@media-id` tokens, but users must already know the exact id. That makes the chat workflow less discoverable than the media-aware editor shown in Palmier.

## Goal

Show compact mention suggestions in the Codex composer when the prompt contains a current `@` token. Selecting a suggestion should insert the canonical `@media-id` token and let the existing resolver target that media.

## Behavior

- `AgentPanel` derives suggestions from the existing `mentionTargets` prop.
- Suggestions appear when the prompt has an unfinished trailing `@` token, such as `@`, `@sam`, or `cut @gen`.
- Matching uses media id, label, and kind, case-insensitively.
- The list is capped to five items and keeps the UI dense.
- Clicking a suggestion replaces the current token with `@media-id `.
- When the inserted token resolves to a different target than the selected/default media, the existing resolved mention row appears.
- Unknown or unmatched tokens keep the current no-suggestion behavior.

## Non-Goals

- No keyboard navigation in this slice.
- No multi-mention backend protocol changes.
- No automatic media selection changes.
- No timeline or generated-asset action changes.

## Testing

- `AgentPanel` shows matching mention suggestions after typing `@`.
- Clicking a suggestion inserts `@media-id`, displays the resolved mention row, and sends the edit request for that media id.
- Suggestions filter by media label as well as id.
- No suggestion list appears when there is no active `@` token.

## Future Work

- Add keyboard navigation and Enter/Tab insertion.
- Add thumbnails or source-kind icons when the composer layout has room.
- Pass multiple referenced media ids to Codex once chat turns support them.
