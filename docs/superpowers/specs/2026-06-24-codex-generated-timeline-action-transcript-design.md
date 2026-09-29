# Codex Generated Timeline Action Transcript Design

## Context

Palmier keeps AI generation actions visible in chat: the assistant can generate, rerun, and tweak
clips while the editor sees tool activity and iteration context. Video Creater now lets the Codex
selected timeline clip block rerun or tweak an AI-generated clip, but those actions only call the
queue callback. They do not add a visible chat entry or tool call, unlike the selected generated
source actions.

## Goal

When a user reruns or tweaks a selected generated timeline clip from the Codex rail, the Codex chat
should record the action as part of the conversation. The editor should be able to see what prompt
was queued, which generated clip it targeted, and that a generation workflow was queued.

## Behavior

- `Rerun same prompt` in the selected timeline clip `AI clip` subsection records:
  - a user entry with the original generated prompt;
  - a loaded `generate_variation` tool entry;
  - a Codex entry saying it queued a rerun for the selected timeline clip.
- `Queue variation` in the same subsection records:
  - a user entry with the trimmed Codex prompt;
  - a loaded `generate_variation` tool entry;
  - a Codex entry saying it queued a variation for the selected timeline clip.
- Chat metadata should include both the selected generated output mention and the timeline clip id
  where possible.
- Existing selected source transcript behavior stays unchanged.
- If the queue callback is unavailable or the prompt is blank, no transcript entry is added.

## Non-Goals

- No new project action, Rust command, Temporal workflow, fal.ai provider behavior, or project
  schema change.
- No automatic timeline replacement after the variation completes.
- No streamed progress UI in this slice. Existing workflow job summaries remain the activity
  surface for queued/running/completed states.

## Validation

- `AgentPanel` tests prove rerunning a selected generated timeline clip appends a visible user
  prompt, `generate_variation` tool call, and Codex confirmation.
- `AgentPanel` tests prove prompt tweaks from the selected generated timeline clip append the same
  chat activity with the trimmed prompt.
- Existing selected source queue transcript tests keep passing.
