# Palmier Latest Render Shell Flattening Design

## Context

The project timeline inspector now presents project identity, format, generated media, and latest export as inline right-rail facts. `Latest render` still sits in a rounded muted panel at the bottom of the same rail, which makes render review feel like a nested utility card instead of a normal project status row.

Palmier keeps editor-side status and asset facts visually direct: the rail should scan as compact project metadata and actions, with cards reserved for repeated history entries or explicit tools.

## Goal

Make `Latest render` match the flattened `Latest export` treatment while preserving the existing render-review data.

## Requirements

- Render `Latest render` as a named region in the project timeline inspector.
- Remove the section's `rounded-md`, `border`, `bg-muted/20`, and `p-2` shell classes.
- Keep the icon, heading, latest render status row, latest render output row, and `No render report` empty state.
- Do not change render report persistence, project schemas, Temporal workflow behavior, preview render review, export artifact handling, or recent export history cards.

## Verification

- Add a `ProjectTimelineInspector` regression test that renders a persisted render report, finds the `Latest render` region, proves the shell classes are absent, and verifies status/output remain visible.
- Run the focused inspector test, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to right-rail presentation chrome.
- Render-review data and durable text-file project workflows remain unchanged.
