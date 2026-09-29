# Palmier Source Reveal Footer Removal Design

## Context

Selected imported timeline clips currently show a revealable imported-source media card and a
separate `Reveal source media` button at the bottom of the Source Inspector. Both controls trigger
the same source reveal action. Palmier-style rails keep the selected source context direct and avoid
duplicated footer buttons when the media row itself is already the source action.

## Goal

Remove the standalone `Reveal source media` footer button from imported source clip inspectors while
preserving the revealable imported-source media card.

## Requirements

- Imported timeline clips with `onRevealSource` keep a clickable `Imported source media <id>` card.
- Clicking that card still calls `onRevealSource(mediaId)`.
- The standalone `Reveal source media` button must not render.
- Generated reference cards and generated output reveal actions remain unchanged.
- Selected library media, timeline trim controls, project files, Temporal workflow data, and fal.ai
  generation behavior are unchanged.

## UI Treatment

The imported-source card is the single reveal affordance. The inspector should move from timeline
metadata into imported source details without an extra footer action. No replacement text, secondary
button, toggle, tab, or menu is introduced.

## Tests

Update `source-clip-inspector.test.tsx` to prove:

- `Reveal source media` is absent for imported timeline clips
- the `Imported source media <id>` card remains present
- clicking the card calls `onRevealSource(mediaId)`

Run the focused source inspector test first, then the full suite before committing.
