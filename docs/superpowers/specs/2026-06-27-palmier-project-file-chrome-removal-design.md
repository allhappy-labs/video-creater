# Palmier Project File Chrome Removal Design

## Intent

Palmier's inspector presents project identity and format facts as editing context. It does not expose schema rows or split-file locations as persistent editor chrome. Video Creater's right rail still shows a `Project files` block with `Schema`, `Directory`, `Timeline`, and `Media` rows, which makes the manual editor feel like a project manifest debugger rather than a timeline editor.

## Requirements

- Remove the visible `Project files` block from the `Project timeline inspector`.
- Replace it with a compact `Project` identity section that shows `Name` and, for split projects, `Path`.
- Keep the project name, split/embedded status badge, duration, track/item counts, render format, media counts, generated media summary, latest export, workflow queue, and latest render sections.
- Do not remove split project support, text-file-editable workflows, MCP setup copy, agent context, validator coverage, or docs that intentionally mention `timeline.json` and `media/index.json`.
- Do not replace the removed block with another always-visible implementation section; visible rows must stay user-facing, not schema/file-split bookkeeping.

## Testing

- Update `ProjectTimelineInspector` tests to assert the right rail no longer renders `Project files`, schema labels, manifest fallback paths, `timeline.json`, or `media/index.json`, while project identity remains visible.
- Update `EditorWorkspace` split-project right-rail coverage to prove split project status, project name, path, duration, and counts remain visible while implementation file rows are absent.
- Run focused inspector/workspace tests, typecheck, full tests, and browser QA for the right rail.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to presentation chrome in the right rail.
- Split project behavior remains available through the existing project files, validation, MCP, and agent setup surfaces.
