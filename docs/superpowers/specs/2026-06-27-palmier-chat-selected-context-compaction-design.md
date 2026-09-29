# Palmier Chat Selected Context Compaction

## Context

Palmier's assistant rail is conversation-first: selected media and timeline state are available to
the agent, but the chat column does not become a second inspector. Video Creater has already removed
the project-context debug panels and selected-source direct action stacks, but the Codex rail still
renders persistent `Selected timeline clip` and `Selected Codex source` regions above the chat
transcript.

Palmier reference: https://www.palmier.io/docs

Those regions duplicate right-rail inspector content and consume the first visible part of the chat
rail. They also keep source-reference open, mention, and prompt-copy controls in a place that should
primarily show conversation and tool activity.

## Goal

Compact selected timeline and selected source context into the Codex transcript and composer state,
so the chat rail opens directly on conversation while preserving agent routing and manual ways to
inspect or mention the selected objects.

## Behavior

- `AgentPanel` no longer renders persistent `Selected timeline clip` or `Selected Codex source`
  regions above the `Codex chat` transcript.
- Default `project_context` tool-call rows remain the visible indication that Codex has selected
  source or timeline context.
- Selected-source variation-set prompts, selected-source referenced generation, selected-source
  upscale routing, and selected timeline edit prompts keep using the existing selected context props.
- Source reference thumbnails, provenance, prompt copy, source reveal, and generated composer handoff
  remain available through the right Source Inspector and media library, not the chat rail.
- Mention insertion remains available from the composer mention suggestions and inspector reference
  controls. The chat rail does not need duplicate reference-thumbnail mention buttons.
- The composer keeps its active target badge and primary action switching for selected generated
  sources and selected generated timeline clips.
- Existing project actions, Temporal-backed generation requests, timeline edit proposals, and split
  project persistence remain unchanged.

## UI Contract

- The first scrollable content in the Codex rail is `Codex chat`.
- Tool-call rows may mention selected context, for example `project_context loaded` with `clip ...`
  or `context @...`.
- The rail should not show selected media filenames, prompts, generated reference thumbnails, track
  lock chips, or source path facts outside transcript/tool rows.
- Manual inspection of selected source details belongs in `Source Inspector`.
- Manual timeline item details belong in the timeline/right-rail inspector surfaces.

## Verification

- `AgentPanel` tests assert `Selected timeline clip` and `Selected Codex source` regions are absent
  while default `project_context` transcript rows still describe the selected context.
- `AgentPanel` tests keep proving selected-source variation sets, referenced generation, upscale,
  and selected generated timeline variation prompts route through composer actions.
- `EditorWorkspace` tests assert the Codex rail no longer duplicates selected source or timeline
  inspector content, while the right Source Inspector and timeline inspector still expose the manual
  details.
- Browser QA captures desktop and narrow rail layouts to confirm the chat transcript starts directly
  below the compact rail header and the composer remains anchored.

## Self-Review

- This is a visible UI compaction only.
- No schema, Temporal, fal.ai, generated asset, media folder, or timeline project action changes.
- The implementation must avoid deleting selected context state; it should only move selected
  context presentation out of persistent chat chrome.
