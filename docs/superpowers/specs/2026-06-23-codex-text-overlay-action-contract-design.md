# Codex Text Overlay Action Contract Design

## Context

Video Creater now has validated project actions for editing text-backed timeline items and for updating manual text overlays with timing, copy, visual treatment, motion, safe zone, and avoid rules. The manual editor can use these actions, but the Codex app-server prompt and structured output schema still only advertise older timeline and template actions. That leaves connected agents less able to perform the same project-file-editable overlay updates that a human can perform in the UI.

Palmier's docs describe connected agents that can adjust clips while seeing full project context. For Video Creater, that means the app-server must expose both the action names and the existing overlay metadata in bounded timeline context.

## Goal

Advertise `editTextItem` and `updateTextOverlayItems` to Codex app-server turns, and include manual text overlay visual metadata in timeline context so agents can revise existing overlays through Rust-validated project actions.

## Behavior

- The app-server ProjectAction guidance lists `editTextItem` and `updateTextOverlayItems`.
- The structured output schema `projectActions.items.properties.type.enum` includes both action types.
- The prompt tells agents to use `updateTextOverlayItems` for manual text overlay timing, copy, visualTreatment, motion, safeZone, and avoid edits.
- Timeline item summaries include overlay text metadata fields when present:
  - `text`
  - `visualTreatment`
  - `motion`
  - `safeZone`
  - `avoid`
  - `textEdited`

## Non-Goals

- No new project action mutation logic; Rust validation already exists.
- No UI changes.
- No Temporal workflow changes. This is a structured proposal contract update, not a queued job.

## Acceptance Criteria

- App-server turn request tests prove the prompt guidance includes the new action names and usage guidance.
- App-server schema tests prove the output schema admits both action names.
- Timeline context tests prove text overlay metadata is visible to agents.
- Existing Codex proposal validation remains unchanged and still validates project actions by applying them to a cloned project.
