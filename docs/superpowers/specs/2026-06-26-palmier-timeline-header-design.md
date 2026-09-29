# Palmier Timeline Header Design

## Intent

Palmier's bottom timeline begins with editing controls and the ruler, not a separate card title. Video Creater still renders a visible `Timeline` heading above the toolbar, which adds vertical chrome and duplicates the selected `Timeline` viewer tab in the center preview. Flatten the bottom timeline so the toolbar is the first visible surface in that panel.

## Requirements

- The center editor must no longer expose a bottom timeline `Timeline` heading.
- The `Timeline tools` toolbar must remain the first accessible timeline control surface.
- Timeline selection context, snapping, split/delete/duplicate/source buttons, source marks, add-track controls, playhead controls, zoom controls, ruler, and canvas behavior must not change.
- The right-rail project timeline inspector may keep its own `Timeline` heading.
- Browser QA must confirm the bottom timeline starts with the compact toolbar row.

## Verification

- Update `EditorWorkspace` tests to prove the center timeline panel has no `Timeline` heading while `Timeline tools` remains present.
- Keep timeline interaction tests passing.
- Browser-smoke the editor and confirm the bottom timeline header is flatter and closer to the Palmier reference.
