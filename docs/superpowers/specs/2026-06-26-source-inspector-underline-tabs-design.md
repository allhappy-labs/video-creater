# Source Inspector Underline Tabs

## Context

Palmier presents `Details` and `AI Edit` in the right source rail as quiet underline
tabs. Video Creater currently uses segmented pill tabs for generated source inspectors.
The behavior is correct, but the heavier control style makes the inspector look more
like a settings panel than an editing rail.

## Goal

Restyle generated Source Inspector `Details` / `AI Edit` tabs to use a Palmier-like
underline treatment while preserving the existing tab behavior, accessible roles, and
source edit workflows.

## Requirements

- Generated Source Inspector tablists use the compact underline tab treatment.
- The selected tab is indicated by stronger text plus a bottom border.
- Unselected tabs stay visually quiet and keep a clear hover/focus state.
- The tab row does not use a segmented pill container or selected filled background.
- The existing accessible tablist and tab names remain unchanged.
- Existing details and AI edit panel content, callbacks, prompt drafts, and insert actions
  remain unchanged.
- Imported visual sources remain inline per
  `2026-06-26-imported-inspector-ai-edit-inline-design.md`; this slice does not add
  imported source tabs.
- No project schema, generation workflow, Temporal, or provider contract changes.

## Verification

- Source Inspector tests assert generated tablists use underline styling.
- Existing Source Inspector behavior tests continue to pass.
- EditorWorkspace integration tests continue to pass.
- Browser QA confirms the right rail tabs read like Palmier underline tabs and do not
  overlap or clip in the inspector rail.
