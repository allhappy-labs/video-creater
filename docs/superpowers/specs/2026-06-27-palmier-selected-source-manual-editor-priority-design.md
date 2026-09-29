# Palmier Selected Source Manual Editor Priority Design

## Goal

When a timeline source clip is selected, the inspector rail should prioritize manual editing controls before read-only/generated source details. This keeps clip trim, source range, and split controls visible without forcing the editor to scroll past generated recipe metadata.

## Design

- Render `Timeline source clip editor` first for selected source clips.
- Keep the read-only `Source Inspector` in the same rail below the manual editor so generated provenance and reference previews remain available.
- Leave non-source selections unchanged: captions, overlays, and standalone timeline context continue to use their existing inspector behavior.
- Do not add new tabs, mode switches, or command rows.

## Testing

- Add a focused DOM-order assertion to the existing selected-source inspector test.
- Run the focused test before implementation to confirm the current source-first order fails.
- Run the focused test, full workspace test file, lint, full tests, and desktop/narrow Playwright visual QA after implementation.
