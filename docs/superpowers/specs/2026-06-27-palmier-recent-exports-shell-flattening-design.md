# Palmier Recent Exports Shell Flattening Design

## Context

`Latest export` and `Latest render` now read as inline project metadata in the right rail. `Recent exports` still wraps its history list in a rounded muted panel, which makes export history look heavier than the surrounding project facts.

Palmier-style editor rails keep section framing quiet and reserve cards for repeated objects. The individual export artifacts are repeated history entries and can remain card-like; the outer history section should not add another nested shell.

## Goal

Flatten the `Recent exports` outer region while preserving export history cards and ordering.

## Requirements

- Keep the `Recent exports` named region directly below `Latest export` when export artifacts exist.
- Remove the outer region's `rounded-md`, `border`, `bg-muted/20`, and `p-2` shell classes.
- Keep the heading, icon, count text, newest-first sorting, three-item cap, and artifact cards.
- Keep artifact card content unchanged: id, path, kind, format, job, and created timestamp.
- Do not change export artifact persistence, split-project files, Temporal workflows, render reports, or export commands.

## Verification

- Extend the `ProjectTimelineInspector` recent exports regression to assert the outer region is flat while artifact cards still render.
- Run the focused inspector test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to the `Recent exports` region shell.
- Repeated export artifact cards remain intentionally framed.
