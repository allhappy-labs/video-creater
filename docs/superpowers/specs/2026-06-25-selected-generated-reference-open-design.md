# Selected Generated Timeline Reference Open Design

## Problem

Palmier-style generated clips are not just timeline items. They are editable AI media with first-frame, last-frame, and reference inputs that need to stay inspectable after the clip lands on the timeline. Video Creater already shows these references in the Codex selected timeline clip rail, but the chips only insert `@media` mentions into the prompt. That makes it harder to verify what visual material drove a generated shot before rerunning or replacing it.

## Goals

- Let editors open a selected generated timeline clip's first-frame, last-frame, and reference media directly from the Codex rail.
- Preserve the existing quick `@media` mention workflow for prompt drafting.
- Reuse the existing source viewer tab system so no new inspection surface is introduced.
- Keep the action scoped to generated timeline clip references; selected source reference chips can keep their current mention-first behavior.

## Non-Goals

- No changes to generation provider contracts, project schema, or generated asset metadata.
- No new reference editing or replacement flow.
- No timeline mutation from opening a reference.

## Design

In the selected generated timeline clip's AI clip block:

- Each reference row has a primary chip action labelled `Open <role> <mediaId>`.
- The primary action calls a workspace callback with the reference `mediaId`.
- The workspace opens/selects that media in the existing source viewer tab via `activateViewerSource(mediaId)`.
- A compact adjacent `@` action remains labelled `Mention <role> <mediaId>` and inserts the same prompt mention as before.

This matches the existing viewer/source architecture and keeps agent-controlled inspection and prompt construction available from the same rail.

## Acceptance Criteria

- AgentPanel calls the open-reference callback when the primary selected timeline reference chip is clicked.
- AgentPanel still inserts `@mediaId` when the compact mention action is clicked.
- EditorWorkspace wires the callback to the source viewer so clicking a generated clip reference opens the corresponding source media tab and source inspector.
- Existing generated clip rerun/replace controls remain unchanged.
