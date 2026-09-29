# Generated Mention Discovery Design

## Context

Palmier chat supports `@` references so an editor can point the assistant at a specific image, look, subject, or frame. Video Creater now has mention suggestions and keyboard navigation, but generated outputs are discoverable only by media id, output filename, or kind. Generated assets already carry better human context: name, prompt, model, output path, and source references.

## Goal

Make Codex mention suggestions discover generated outputs from generated-asset context, so editors can type terms like `@hero` or `@cinematic` and insert the canonical generated output media id.

## Behavior

- Generated output mention targets use the generated asset name as the primary label when present.
- Generated output mention targets keep the output filename/path available as secondary detail.
- Mention matching searches media id, primary label, kind, output path, generated asset id, generated asset prompt, model provider/id, and referenced media ids.
- Inserting a suggestion still writes the canonical `@media-id ` token.
- Typed full media ids continue resolving exactly as before.
- Imported media mention targets keep their current filename labels.

## Non-Goals

- No multi-mention request protocol changes.
- No prompt parsing beyond the existing active trailing `@` token.
- No backend, Temporal, or project schema changes.

## Testing

- `EditorWorkspace` shows a generated output suggestion when the query matches the generated asset prompt.
- Pressing Enter inserts the generated output media id, not the human label.
- The suggestion visually includes the generated asset name and output filename.
- Existing exact generated media id mention tests keep passing.
