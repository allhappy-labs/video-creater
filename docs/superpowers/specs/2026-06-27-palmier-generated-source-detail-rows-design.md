# Palmier Generated Source Detail Rows Design

## Context

Generated source details still split basic metadata into nested `Generation` and `Format` groups.
Imported source details were already flattened into one row group, and Palmier-style editor rails keep
selected source facts close to the media instead of presenting them as settings subsections.

## Goal

Render generated source file facts as one dense `Generated file details` group. Keep all current
facts, generated references, prompt, and AI edit actions, but remove the nested `Generation` and
`Format` subsection headings.

## Requirements

- `Generated file details` remains the accessible group for generated source metadata.
- Rows include model, status, aspect, path, type, duration, resolution, and frame rate.
- Nested `Generation` and `Format` groups must not render.
- The words `Generation` and `Format` must not appear as subsection headings inside generated file
  details.
- Generated references, output reveal cards, prompt copy, variation controls, rerun actions,
  replacement actions, project files, Temporal workflow data, and fal.ai model defaults remain
  unchanged.

## UI Treatment

Use the existing compact key/value `detailRow` treatment. The generated details rail should read as
direct source context:

- no cards or nested panels inside the details group
- no duplicate subsection labels
- row separators remain lightweight
- values stay right-aligned and wrapping-safe

## Tests

Update `source-clip-inspector.test.tsx` and the relevant `editor-workspace.test.tsx` assertion to
prove:

- generated file details contain all eight metadata rows directly
- `Generation` and `Format` nested groups are absent
- generated references and prompt remain visible

Run focused source-inspector and workspace tests first, then the full suite before committing.
