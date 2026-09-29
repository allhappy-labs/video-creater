# Preview Active Viewer Kind Badge Design

## Intent

Palmier treats the center viewer as a tabbed workspace: `Timeline` and each opened source are separate contexts. Video Creater can keep generated source tabs open while the timeline tab is active, but the preview header should describe the active viewer context, not a background source tab. This prevents an open generated source from making the timeline viewer read as `AI`.

## Requirements

- Show the preview source-kind badge only when the active viewer is `source` and a source is selected.
- Hide the source-kind badge when the active viewer is `timeline`, even if generated source tabs remain open.
- Preserve the existing `AI` source tab chip for generated source tabs.
- Preserve the `AI` preview header badge when the active source viewer is selected.
- No timeline, source-tab ownership, generation, Temporal, project schema, or render behavior changes.

## Verification

- Add a `PreviewPanel` regression test for timeline mode with an open generated source tab.
- Existing preview header, generated provenance, source viewer, and viewer tab tests continue to pass.
