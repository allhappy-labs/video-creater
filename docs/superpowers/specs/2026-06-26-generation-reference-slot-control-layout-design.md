# Generation Reference Slot Control Layout Design

## Context

Palmier's video generation panel presents first frame, last frame, and reference inputs as readable
visual targets. Video Creater now has a focused generation sheet, but the first/last/reference slot
picker row still places the select, `Use selected`, and remove action side by side. In the focused
sheet this can shrink the media picker to a tiny width, making the selected file hard to read.

Palmier reference: https://www.palmier.io/docs

## Goal

Make generation reference slot controls scan like editor inputs: the picker keeps full slot width
and the secondary actions sit below it.

## Behavior

- `First frame`, `Last frame`, and general `Reference` slots expose a named controls group.
- The media picker select spans the available slot width.
- `Use selected` and remove actions render in a compact action row below the picker.
- The existing selected-reference preview rows, drag/drop handlers, remove actions, and submit
  payloads remain unchanged.
- Audio mode remains without visual reference controls.

## Non-Goals

- No schema, Temporal, fal.ai, or request contract changes.
- No new media picker or batch selection behavior.
- No change to first/last/reference semantics.

## Verification

- Add a `MediaBin` test proving first/last/reference controls use stacked groups and no longer put
  action buttons in the same row as the select.
- Existing media generation reference and payload tests continue to pass.
- Browser-smoke the open generation sheet and confirm the first frame select is visibly full-width
  in its slot.
