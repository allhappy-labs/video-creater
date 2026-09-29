# Compact Generated Source Preview Provenance Design

## Context

Palmier keeps source viewing central while making AI provenance inspectable through source tabs,
the right inspector, and compact generated-asset context. Video Creater already opens source tabs,
switches the right rail to source details, and shows generated source provenance above the preview.
That provenance is currently table-like and vertically heavy, which pushes the viewer and timeline
down when a generated source is selected.

Palmier reference: https://www.palmier.io/docs

## Goal

Make generated source provenance in the preview header compact and scan-friendly so the viewer and
timeline remain the dominant surfaces.

## Behavior

- Keep the selected source preview details region.
- Keep showing generated asset id, model, prompt, and references when present.
- Render generated provenance as a compact horizontal chip/list strip instead of stacked detail
  rows.
- Preserve accessible structure with a named provenance group and a named list of provenance
  items.
- Truncate long values visually while preserving full values in titles and accessible labels.
- Keep the full detailed generated provenance and AI-edit controls in the right Source Inspector.

## Non-Goals

- No changes to generation, Temporal workflow, fal.ai, media loading, or project file behavior.
- No change to source tab selection semantics.
- No removal of preview source metadata, render review, or transport controls.
- No broad theme rewrite.

## Verification

- Preview panel tests prove generated provenance renders as a compact named list.
- Existing preview panel source-tab and source-viewer tests continue to pass.
- Browser QA checks desktop and narrow widths for non-overlapping preview, source tabs, transport,
  and timeline.
