# Palmier Inspector Duplicate Switch Removal Design

## Intent

Palmier's right rail presents one inspector surface with a compact context label and local inspector tabs. Video Creater currently renders two stacked controls when source and timeline context are both available: an `Inspector context` card with `Source`, `Inspector`, and a viewer switch button, followed by a separate `Source` / `Timeline` inspector tablist. This adds chrome that Palmier does not show and duplicates the center viewer tabs.

## Requirements

- Remove the standalone `Inspector context` group from the right rail when a source inspector exists.
- Remove the compact right-rail `Show timeline viewer` / `Show source viewer` button. The center viewer tab strip remains the way to switch the viewer between timeline and opened sources.
- Keep one accessible `Inspector rail views` tablist for switching right-rail inspector content between `Source` and `Timeline`.
- Style the inspector rail tablist as a Palmier-like underline header rather than boxed segmented buttons.
- Source inspector content, project timeline inspector content, source selection, media reveal, and center viewer tab behavior must not change.
- When no source inspector exists, the timeline inspector continues to render directly without the source/timeline tablist.

## Testing

- Update `EditorWorkspace` tests to assert the duplicate `Inspector context` group is absent.
- Keep coverage proving the `Inspector rail views` tabs switch between Source Inspector and Project Timeline Inspector.
- Update viewer-switch coverage to use the center viewer tabs instead of a right-rail viewer button.
- Run focused workspace tests, typecheck/lint, and browser QA for the right rail.
