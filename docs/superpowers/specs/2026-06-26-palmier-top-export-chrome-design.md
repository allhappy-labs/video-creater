# Palmier Top Export Chrome Design

## Intent

Palmier keeps the top-right app chrome focused on `Export` instead of exposing multiple render toggles and duplicate render buttons beside it. Video Creater currently shows `Draft WebM`, `Final WebM`, `Render draft`, and `Render final` in the top bar even though the Export menu already contains Draft WebM and Final WebM actions. Simplify the top bar so manual editing has less non-Palmier chrome while keeping export functionality available.

## Requirements

- The workspace top action bar keeps `Import` and a single `Export` button.
- The top action bar no longer renders the standalone Draft/Final WebM segmented control.
- The top action bar no longer renders standalone `Render draft` or `Render final` buttons.
- Draft WebM and Final WebM remain available from the Export menu.
- Existing render report generation still works through the Export menu actions.
- No render profile schema, Temporal workflow, project action, or file export behavior changes.

## Verification

- Add/update `EditorWorkspace` tests proving top chrome excludes duplicate render controls.
- Update render-report tests to use the Export menu Draft/Final WebM actions.
- Existing export-menu tests continue to pass.
- Browser-smoke the desktop editor and confirm the top-right chrome is less crowded.
