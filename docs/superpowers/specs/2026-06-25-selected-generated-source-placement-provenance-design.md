# Selected Generated Source Placement Provenance

## Context

Palmier treats generated media as inspectable editor context: users can inspect the prompt, first
frame, last frame, references, and generation details from the source view. Video Creater already
shows prompt, references, file metadata, and generation settings for the selected generated source,
but it does not show project placement metadata such as timeline intent or folder destination.

Palmier reference: https://www.palmier.io/docs

## Goal

Make selected generated source details preserve the project context that explains where the
generation was meant to land.

## Behavior

- In selected generated source `Details`, show a compact `Project` section when placement metadata
  is present.
- `placementIntent: "timeline"` renders `Placement` as `Timeline`.
- `placementIntent: "library"` renders `Placement` as `Library`.
- `placementIntent: "replace:<itemId>"` renders `Placement` as `Replacement`.
- A known `targetFolderId` renders `Destination` using the same nested folder label format as the
  media bin, such as `Generated selects / Scene A`.
- Generated sources without placement or known destination metadata keep the existing details layout.

## Non-Goals

- No project schema, Temporal workflow, or fal.ai provider change.
- No new generated source action.
- No automatic folder navigation or timeline selection behavior.

## Verification

- Media Bin test: selected generated source details show timeline placement and nested folder
  destination when the selected generated asset carries that metadata.
- Existing selected generated source prompt, references, workflow, replacement, insertion, and
  composer tests continue passing.
