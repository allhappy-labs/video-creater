# Export File Type Row Removal

## Goal

Remove the read-only `File Type` row from the video export dialog because it is not a user-selectable setting.

## UI behavior

- Keep `Codec`, `Quality`, `Resolution`, and `Frame Rate` in the dialog settings area.
- Remove the standalone `File Type` setting row.
- Keep the selected format and extension in the dialog footer, such as `WebM (.webm)`, where it serves as output summary metadata rather than appearing to be a control.
- Do not change codec selection, export payloads, capability gating, or render behavior.

## Verification

- Add a focused regression assertion that the dialog has no `File Type` row.
- Keep coverage proving the footer still shows the selected format and extension.
- Update browser visual-QA assertions and parity wording so they no longer require a visible `File Type` row.
- Run the focused export-sheet tests, browser-QA source test, TypeScript lint, and refresh the export-dialog screenshot.
