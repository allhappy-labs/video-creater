# Collapsible Generated Source Media Panel Design

## Context

Palmier keeps the media library scannable while generated source provenance, prompt, references,
and AI edit controls remain nearby. Video Creater already exposes those generated-source controls
inside the media panel, but selecting a generated output expands a long details surface above the
library grid. That makes the selected source easy to inspect, but it pushes source footage,
generated selects, and audio further down the panel.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep generated-source inspection available in the media panel without letting it dominate the media
library. A selected generated output should show a compact summary first, with full Details and AI
Edit controls available through an explicit expand action.

## Behavior

- The selected generated source section keeps its existing role and title.
- The header continues to show title, AI status, output path, and generated status.
- The primary actions `Insert on timeline`, `Rerun same prompt`, and `Use in composer` stay visible
  when available.
- Full details, references, workflow, prompt, and AI-edit variation controls render only when the
  section is expanded.
- The details toggle uses a clear accessible label and exposes pressed/expanded state.
- Selecting a different generated source resets the section to collapsed details mode.
- Existing right-rail source inspection remains unchanged and continues to provide the full
  detailed view.

## Non-Goals

- No generation, Temporal, fal.ai, media-file, or project persistence behavior change.
- No removal of any selected generated source action or provenance data.
- No redesign of project folder browsing, source tabs, or the right Source Inspector.

## Verification

- Media bin tests prove selected generated source details are collapsed by default.
- Media bin tests prove expanding the section reveals generated details and AI Edit still exposes
  variation controls.
- Existing media-bin generation, source selection, replacement, and workflow tests continue to pass.
- Browser QA checks desktop and narrow widths for non-overlapping selected source, library grid, and
  action buttons.
