# Palmier Generation Reference Action Labels Design

## Context

Video Creater's generation composer already supports first frame, last frame, and additional visual
references through selectable media controls and preview tiles. The remaining Palmier mismatch is
that selected reference remove actions expose internal media IDs, so the composer still feels partly
like an editable project file instead of a visual media editor.

Palmier reference: https://www.palmier.io/docs

## Goal

Use visible asset filenames for selected generation reference action labels while preserving media IDs
for request payloads, de-duplication, and project-file references.

## Behavior

- Selected reference preview tiles continue to render filenames, metadata, thumbnails, and remove
  buttons.
- Multi-reference remove buttons use the visible filename in their accessible label.
- First/last frame remove labels remain slot based because each slot has only one selected item.
- Unknown or missing media assets may still fall back to the stored media ID.
- Submitted generation payloads remain unchanged: references still contain stable media IDs.

## Non-Goals

- No schema, Temporal, fal.ai, or generated asset contract changes.
- No change to dropdown option values or drag-and-drop payloads.
- No redesign of the composer layout.

## Verification

- Add `MediaBin` coverage with filenames that do not contain media IDs.
- Confirm the visible remove action uses the filename and the old media-ID label is absent.
- Confirm the queued generation request still submits the media ID.
