# Palmier Generation History Icon Design

## Context

The media generation sheet has moved closer to Palmier's compact composer, but the header still
renders a visible `History` text button next to the credit and close controls. Palmier's reference
composer keeps this header area dense: the credit balance, history, and close actions are compact
icon controls, leaving the panel width for generation modes, references, and prompt content.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the generation history control read as compact editor chrome instead of a secondary text
button.

## Behavior

- The media generation sheet still exposes a `Generation history` button.
- The button still toggles the generation history region.
- The button keeps its pressed state when history is open.
- The visible header no longer renders the text `History`.
- The control uses an icon-only footprint aligned with the close button.
- Credit balance, estimated cost, active generation count, mode selection, references, prompt, and
  generation request payloads remain unchanged.

## Non-Goals

- No generation history data model changes.
- No queue, Temporal, fal.ai, or generated asset behavior changes.
- No redesign of the full generation composer.

## Verification

- Update `MediaBin` coverage to assert the history action is icon-only while retaining its accessible
  name and toggle state.
- Keep generation composer and request tests passing.
- Browser QA the open generation sheet to confirm the header remains compact and the history action
  can still open the history region.
