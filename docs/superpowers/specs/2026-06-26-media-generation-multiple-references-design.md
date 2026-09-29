# Media Generation Multiple References Design

## Context

Palmier's generation workflow treats reference images as a plural input next to first and last
frame control. Video Creater's project schema already stores generated media references as
`references.mediaIds: string[]`, but the media generation composer only lets editors choose one
general `Reference`. This blocks an editor from queueing a shot with multiple style, subject, or
composition references even though the Temporal/fal request shape already supports it.

Palmier reference: https://www.palmier.io/docs

## Goal

Let editors attach multiple general visual references to a media generation request from the Media
panel composer while preserving existing first-frame and last-frame behavior.

## Behavior

- Video mode keeps the existing `First/Last` and `Reference` tabs.
- `First frame` and `Last frame` remain single-select slots.
- The `Reference` tab becomes an add-and-review list:
  - choosing a visual media asset appends it to the reference list;
  - duplicate selections are ignored;
  - selected references render as compact preview rows with filename, metadata, and a remove action;
  - removing one row leaves the other references in order.
- Image mode uses the same multi-reference list without first/last frame slots.
- Audio mode continues to hide visual references and submits an empty reference list.
- Composer seeding from selected generated output provenance, history, and Source Inspector handoff
  restores all compatible `references.mediaIds`, not only the first one.
- Dragging media onto the reference list appends it; dragging onto first/last frame still replaces
  the single slot.
- Submit sends the existing request shape with all selected reference IDs in `references.mediaIds`.

## Non-Goals

- No project schema change.
- No Temporal workflow contract change.
- No fal.ai provider behavior change.
- No multi-select media picker or batch folder import changes.
- No change to first/last frame semantics.

## Verification

- MediaBin tests cover appending, de-duping, removing, and submitting multiple references.
- Existing generation history and selected generated source composer tests prove all provenance
  references are restored.
- Existing workspace generation tests continue to pass with the unchanged request contract.
