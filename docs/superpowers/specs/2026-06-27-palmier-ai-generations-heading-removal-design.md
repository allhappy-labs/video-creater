# Palmier AI Generations Heading Removal Design

## Goal

Make the source library read more like one dense media browser by removing the visible `AI generations` section title above generated asset cards. Palmier's media browser treats generated outputs as first-class assets with compact badges and cards, not as a separately titled log panel.

## Design

- Keep the generated assets region accessible as `AI generations`.
- Keep the generated card grid accessible as `AI generation grid`.
- Remove the visible `AI generations` heading row and sparkle icon.
- Preserve generated asset cards, previews, output selection, composer reuse, replacement, insertion, lineage, workflow status, and mock completion actions.

## Testing

- `MediaBin` proves the accessible `AI generations` region remains while the visible heading text is absent.
- `EditorWorkspace` proves generated asset provenance still renders in the source library without the repeated visible title.
- Existing generated asset card, workflow status, selection, replacement, insertion, and mock-generation tests continue to cover behavior.
