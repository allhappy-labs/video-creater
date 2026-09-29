# Palmier Inline Video Reference Controls Design

## Context

Palmier documents video generation as one flow: choose first and last frames, add reference images
for style or subject consistency, then regenerate or swap clips without leaving the project. Video
Creater still hides those controls behind a `First/Last` versus `Reference` segmented switch inside
the media generation sheet. That switch is extra chrome and makes either the frame anchors or the
reference picker disappear.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the video generation reference switch and show first frame, last frame, and reference-image
controls together in one compact section.

## Behavior

- Video mode no longer renders `Generation reference controls`, `First/Last`, or `Reference` tab
  buttons.
- Video mode renders the existing `Generation first and last frame slots` group followed by the
  existing `Reference slot` group.
- Filled First/Last slots keep their compact media attachment chrome; empty slots keep their picker
  and selected-media shortcut.
- The reference picker remains available without changing modes, and selected references still
  render as removable visual chips.
- Image mode keeps the existing reference-only picker.
- Audio mode still hides visual reference controls.
- Generation request payloads, reference IDs, drag/drop behavior, prompt handling, model settings,
  Temporal records, fal.ai provider settings, and folder placement remain unchanged.

## Verification

- `MediaBin` tests prove video mode shows first/last frame slots and the reference slot together
  without the reference segmented switch.
- Existing generation request tests are updated to choose reference media without first clicking a
  `Reference` tab.
- Existing image and audio mode reference visibility tests keep passing.
- Browser QA confirms the open generation sheet has one continuous reference area with no hidden
  tab switch and no overlapping controls.
