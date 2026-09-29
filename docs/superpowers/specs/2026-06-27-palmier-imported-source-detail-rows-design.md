# Palmier Imported Source Detail Rows Design

## Context

The Source Inspector already uses a flatter rail shell, but imported library media still splits basic file facts into nested `Project` and `Format` subsections. Palmier-style editor rails keep source facts close to the selected media instead of presenting them as settings panels. The current subsection labels add visual weight without adding an editing decision.

## Goal

Render imported source file facts as one dense details group in the Source Inspector. Keep the insert and AI edit actions unchanged, but remove the `Project` and `Format` subsection headings so the rail reads as direct source context.

## Requirements

- Imported media details must remain in the `Imported file details` group.
- The rows must include name, path, type, duration, resolution, and frame rate.
- The `Project` and `Format` subsection headings must not render for imported source details.
- Existing insert-on-timeline, referenced generation, and upscale actions must keep their labels and callbacks.
- The change is visual only; no project schema, media model, or Temporal workflow behavior changes.

## UI Treatment

The details group uses the existing compact key/value row treatment:

- uppercase labels are avoided inside imported source detail rows
- row borders remain as lightweight separators
- values remain right-aligned and wrapping-safe
- no cards, nested panels, tabs, toggles, or extra switches are introduced

## Tests

Add a regression test in `source-clip-inspector.test.tsx` that renders an imported source and verifies:

- the `Imported file details` group contains all six file facts
- `Project` and `Format` are absent from that group
- the group itself does not contain nested labeled groups for those subsection headings

Run the focused source inspector test first, then the full suite before committing.
