# Palmier Imported Source Inspector Metadata

## Goal

Make the right inspector for selected imported library media read like Palmier's plain metadata sidebar instead of duplicating the media browser thumbnail card. The preview remains in the source library; the inspector focuses on selected-object identity, project path, and format details.

## Requirements

- Keep the `Source Inspector` region flat, without card, border, or background shell classes.
- For selected imported media, keep the filename header and `Library media` context.
- Do not render the `Imported source media <id>` preview-backed card in the inspector for selected library media.
- Render imported metadata as two flat read-only groups:
  - `Project`: `Name` and `Path`.
  - `Format`: `Type`, `Duration`, `Resolution`, and `Frame Rate`.
- Keep `Imported source details` and `Imported file details` accessibility labels available for tests and assistive technology.
- Keep insert, reveal, referenced generation, and upscale actions unchanged.
- Keep generated-source provenance cards unchanged in this slice because they support first-frame, last-frame, reference, and output review.

## Non-Goals

- No timeline, schema, Temporal, fal.ai, or media import changes.
- No source-library thumbnail changes.
- No generated asset inspector redesign.

## Testing

- `SourceClipInspector` verifies selected imported library media has flat `Project` and `Format` metadata groups.
- `SourceClipInspector` verifies the imported preview card is absent while file details and AI edit actions remain available.
- Existing generated provenance tests continue to verify generated first-frame/reference/output cards.
- Browser QA checks the right inspector at desktop and narrow widths for no duplicated imported thumbnail card and no horizontal overflow.
