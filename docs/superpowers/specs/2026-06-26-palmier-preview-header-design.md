# Palmier Preview Header Design

## Intent

Palmier's center viewer uses the viewer tab strip as the header: `Timeline` and open source tabs sit directly above the preview, and the image area stays visually dominant. Video Creater still adds a separate `Preview` title row and an extra source-kind badge above the tabs. That duplicates information already present in the viewer tabs and makes the center column feel more card-like than editor-like.

## Requirements

- The preview panel header must no longer render a separate visible `Preview` title.
- The preview panel header must no longer render the standalone `Preview source kind ...` badge.
- The `Viewer tabs` tablist remains the top header surface for the preview panel.
- Timeline and source viewer tabs keep their existing labels, selected state, close buttons, and previous/next navigation controls.
- The selected source details region, preview viewport, transport controls, and render report behavior do not change.
- The preview panel should keep compact spacing so the viewport starts closer to the top of the center column.

## Verification

- Update `EditorWorkspace` tests to prove the center panel no longer exposes a `Preview` heading or standalone preview source-kind badge.
- Keep existing viewer tab switching and close behavior passing.
- Browser-smoke the editor and confirm the center viewer has a flatter, Palmier-like tab-strip header.
