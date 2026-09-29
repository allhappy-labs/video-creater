# Palmier Generated Prompt Preview Flattening Design

## Goal

Make generated prompt provenance in the right inspector read like direct Palmier metadata instead of a card nested inside generated details.

## Design

- Keep the generated prompt visible in `Generated details`.
- Preserve the `Copy generation prompt` icon button and clipboard behavior.
- Expose the prompt preview as an accessible `Generated prompt` group.
- Remove the prompt preview's rounded border and background shell.
- Keep existing generated file, reference, recipe, AI edit, queue, Temporal, and fal.ai behavior unchanged.

## Testing

- `SourceClipInspector` verifies the prompt group is accessible, flat, and still contains the prompt text plus copy action.
- Existing prompt copy and generated provenance tests continue to cover behavior and data visibility.
