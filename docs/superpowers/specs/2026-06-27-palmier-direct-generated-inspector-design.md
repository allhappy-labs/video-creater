# Palmier Direct Generated Inspector Design

## Goal

Remove the generated source inspector's `Details` / `AI Edit` tab switch so generated media context and regeneration controls are visible in one direct inspector flow. This keeps the rail closer to Palmier's compact side-panel model and removes another mode toggle from manual editing.

## Design

- Render `Generated details` directly below the generated recipe summary.
- Render `Generated AI edit` directly below the generated details section.
- Remove generated inspector tab state, tablist markup, and tab-click behavior.
- Preserve generated output replacement, timeline insertion, reference reveal, prompt copy, rerun, replacement rerun, upscale, variation prompt, and queue actions.
- Keep imported source AI edit behavior unchanged.

## Testing

- Update source inspector tests to assert generated tabs are absent and both generated sections are visible.
- Keep existing action tests for rerun, replacement rerun, prompt edits, upscale, and generated provenance.
- Run focused source-inspector tests, full source-inspector and workspace tests, lint, full tests, and Playwright desktop/narrow visual QA.
