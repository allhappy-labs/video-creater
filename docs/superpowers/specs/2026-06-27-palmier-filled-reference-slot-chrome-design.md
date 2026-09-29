# Palmier Filled Reference Slot Chrome Design

## Context

Video Creater's First/Last generation reference slots still show a select box and action row even
after a frame has already been chosen. Palmier's composer reference slots read as compact media
attachments: a label, a thumbnail, and a small remove control. Filled slots should prioritize the
visual reference, not the form controls used to choose it.

Palmier reference: https://www.palmier.io/docs

## Goal

Make filled First/Last frame slots look like compact media attachments while preserving the existing
selection path for empty slots.

## Behavior

- A filled First frame or Last frame slot still exposes its slot group and remove button.
- A filled slot no longer renders the visible select picker for that same slot.
- A filled slot still renders its visual preview tile, filename, metadata, and duration.
- Empty First/Last slots keep their select picker and selected-media shortcut so the user can choose
  a reference without drag and drop.
- Existing request payloads, reference IDs, preview URLs, and remove behavior remain unchanged.

## Non-Goals

- No change to the Reference tab's multi-reference picker.
- No drag-and-drop behavior changes.
- No media schema, fal.ai, Temporal, or generated asset behavior changes.

## Verification

- Update `MediaBin` coverage so a prefilled First frame slot shows the preview and remove control
  without the `Generation first frame` select.
- Keep empty Last frame coverage proving the picker remains available.
- Keep generation reference request tests passing.
- Browser QA the open generation composer and confirm the filled First frame slot is visibly compact.
