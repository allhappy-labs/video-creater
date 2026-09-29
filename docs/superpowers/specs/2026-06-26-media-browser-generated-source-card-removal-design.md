# Media Browser Generated Source Card Removal Design

## Context

Palmier keeps the left media column focused on browsing folders, generated outputs, and source assets. Video Creater still rendered a selected generated-source card above the media browser, duplicating the right Source Inspector and Codex selected-source context with another details disclosure, preview, prompt, references, workflow metadata, and AI edit controls.

Palmier reference: https://www.palmier.io/docs

## Decision

Remove the selected generated-source card from `MediaBin`. The media column should stay a compact browser: project library, selected media tile state, folder actions, and the AI generations list remain there. Generated-source details, references, prompt, rerun actions, replacement actions, and variation controls remain in the right Source Inspector or Codex selected-source context.

## Requirements

- Selecting a generated output must not render a `Selected generated source` region in the left media browser.
- The selected generated output remains visibly selected as a media tile or generated output card.
- Existing generated output card actions remain available: replacement, timeline insertion, prompt reuse, mock completion/failure, and retry.
- Right Source Inspector and Codex selected-source workflows remain responsible for generated-source prompt, references, workflow status, and variation actions.
- `MediaBin` no longer owns selected generated-source tabs, detail disclosure state, reference viewer buttons, or variation-set UI.

## Testing

- `MediaBin` proves selected generated-source details are absent from the media browser column while the selected output remains visibly selected.
- Existing media-bin generated output, composer seed, replacement, insertion, mock completion, and retry tests continue to pass.
- Existing editor workspace source-inspector and Codex selected-source tests continue to pass.
- Browser QA checks that the left media column begins with project library content and no longer shows the selected generated-source details card.
