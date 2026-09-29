# Named Generated Assets Design

## Context

Palmier's generation composer includes a `Name (optional)` field, and its library/history surfaces show generated assets as organized project items rather than anonymous prompt blobs. Video Creater records strong generated-asset provenance in text-editable project files, but generated assets only have ids, prompts, model settings, references, and outputs. Editors cannot name a generated shot from the composer, and agents reading project files cannot distinguish a short human label from a long prompt.

## Goal

Add optional generated asset names that flow from the Media Bin composer into the canonical project action and generated asset project files.

## Behavior

- The Media Bin generation composer shows a compact `Generation name` input above the prompt.
- Submitting a generation sends `name` as the trimmed value, or `null` when the field is blank.
- `EditorWorkspace` includes the request name in the `recordGeneratedAsset` project action.
- Rust accepts `name` on `recordGeneratedAsset`, trims it, stores `None` for blank names, and serializes it only when present.
- Existing generated assets without `name` remain valid.
- Media Bin generated history, generated asset cards, and selected generated source details prefer `name` for the primary title while still showing the prompt for provenance.

## Visual Treatment

- Keep the name input compact and editor-native: one line, no explanatory helper text.
- Generated asset cards show the name as the bold title and the prompt as secondary text when both are present.
- Selected generated source details show the name near the top and keep the full prompt card unchanged.

## Verification

- Media Bin tests prove named generation requests include a trimmed `name` and blank names emit `null`.
- Editor workspace tests prove queued generation project actions include the request name.
- Rust project action tests prove `recordGeneratedAsset` preserves trimmed names and treats blank names as absent.
- Browser QA checks the composer and generated-source panel remain readable at desktop and narrow widths.
