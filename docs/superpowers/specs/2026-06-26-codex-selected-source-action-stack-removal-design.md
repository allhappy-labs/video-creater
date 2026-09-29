# Codex Selected Source Action Stack Removal Design

## Context

The Codex rail selected-source block duplicates many direct editor actions: insert on timeline, replace selected clip, queue variation, queue replacement variation, queue variation set, use in composer, queue referenced shot, and queue upscale. Palmier's editor keeps the chat rail focused on conversation, project context, and prompt-driven actions, while direct media and timeline edits live in media cards, the timeline, and the inspector.

## Goal

Keep the Codex selected-source block as concise context for the active media source and remove its stacked direct-action buttons.

## Behavior

- The selected-source block still shows the source label, kind, model/settings chips, placement/destination context, references, mention buttons, and prompt copy.
- The selected-source block no longer renders direct action buttons for inserting, replacing, variation queueing, variation sets, composer handoff, referenced generation, or upscale.
- Mention-based composer actions remain available when the prompt explicitly references a media item, such as `Insert mention on timeline` and `Queue referenced shot from mention`.
- Explicit selected-source variation-set prompts still route through the primary composer button, preserving the existing Temporal-backed queue path without a duplicate button in the selected-source block.
- Direct source/media actions remain available in the Source Inspector and media surfaces where they are easier to scan and less likely to conflict with chat context.

## Non-Goals

- Do not change Temporal workflow names, task queues, project action payloads, or generated asset schemas.
- Do not remove Source Inspector AI edit actions.
- Do not remove selected-source references, prompt copy, or mention insertion.
- Do not redesign the full Codex rail layout.

## Verification

- `AgentPanel` tests assert the selected-source region no longer exposes the duplicate action buttons while keeping references and prompt copy.
- Existing composer tests prove mention-based insertion, referenced generation, and selected-source variation-set routing still work.
- Workspace tests prove Source Inspector/media actions still queue Temporal-backed project actions.
- Browser QA confirms the Codex selected-source region reads as concise context and no longer displays the action stack.
