# Codex Selected Timeline Action Stack Removal Design

Palmier's chat rail is conversation-first: it can see the selected timeline item, but it does not turn the chat column into a second inspector full of clip controls. Video Creater already removed the selected-source action stack from the Codex rail. The remaining mismatch is the selected timeline clip context, which still exposes rerun, replacement, reorder, opacity, audio, trim, and split controls directly inside chat.

## Goal

Keep the Codex selected timeline clip block as compact context for agent work, while moving direct edits back to the primary composer, timeline, and inspector surfaces.

## Behavior

- Preserve the `Selected timeline clip` context block.
- Preserve clip identity, kind, timeline range, duration, track name, lock state, visibility state, source media mention, source range, generated model/settings metadata, placement, destination, references, and prompt copy.
- Remove direct rail action groups from that block:
  - imported clip `AI edit` queue buttons
  - generated clip prompt draft plus rerun, replacement, variation, and replacement variation buttons
  - sequence move earlier/later buttons
  - audio fade and volume inputs/buttons
  - visual opacity input/button
  - trim range inputs/button
  - split input/button
- Keep existing prompt-driven composer routes for explicit selected-clip actions, including split, trim, delete, reorder, track state, audio fade/volume, visual opacity, overlay/caption/template edits, and generated clip variations.
- Keep manual editing paths in the timeline toolbar, source viewer, media cards, and right inspector where they already exist.

## Testing

- Add an AgentPanel regression test that renders a selected generated visual clip with all relevant callbacks and asserts that the selected clip context remains visible but the duplicate action buttons and edit inputs are absent.
- Update legacy AgentPanel tests that clicked removed rail controls to assert the new absence where appropriate, while keeping the existing primary-composer tests for the same edits.
- Run the focused AgentPanel test suite, TypeScript checks, diff whitespace check, and desktop/narrow browser QA.
