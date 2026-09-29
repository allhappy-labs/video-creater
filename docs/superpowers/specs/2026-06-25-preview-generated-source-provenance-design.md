# Preview Generated Source Provenance Design

## Context

Palmier makes generated media feel native to the editor: a user can open a generated source, inspect the prompt and references, then rerun or tweak it without leaving the project. Video Creater already exposes generated provenance in the right source inspector and Codex context, but the central preview/source viewer only shows file path and format metadata. When the user focuses on the preview, the generated source loses its model, prompt, and reference context.

## Requirements

- When the active preview source is generated media, show a compact provenance block in the preview details.
- Include the generated asset ID, model/provider label, prompt, and selected reference roles when available.
- Keep imported media unchanged; the provenance block should not render for non-generated sources.
- Preserve existing viewer tabs, preview transport, source path, and source range behavior.
- Use data already present in project JSON (`generatedAssets`, outputs, references); do not change project actions or workflow payloads.

## Design

- Extend `PreviewSource` with optional generated provenance fields:
  - `generatedAssetId`
  - `generatedModelLabel`
  - `generatedPrompt`
  - `generatedReferences`
- Derive these fields in `selectedPreviewSource` by matching the active media ID to a generated asset output.
- Render a compact `Generated source provenance` group inside `PreviewPanel` below the path/range details.
- Keep rows dense and truncating: asset, model, prompt, references.

## Acceptance

- Direct `PreviewPanel` rendering with generated provenance shows the provenance group.
- Opening a generated timeline clip in `EditorWorkspace` shows model, prompt, and reference labels in the central preview details.
- Imported source preview details remain unchanged and do not show generated provenance.
