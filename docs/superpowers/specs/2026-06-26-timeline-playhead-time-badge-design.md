# Timeline Playhead Time Badge Design

## Intent

Make the current timeline position readable directly where editors align clips, captions, overlays, and generated media. Palmier-style timeline work keeps the playhead, time ruler, and clip boundaries visible together; Video Creater should do the same without forcing the editor to look back to the toolbar readout.

## Requirements

- Render a compact timecode badge aligned to the playhead in the timeline ruler band.
- Keep the badge above the editable track rows so it does not cover clip labels, badges, handles, or media thumbnails.
- Use the existing `formatTimecode` display so fractional positions remain precise.
- Update the badge whenever the playhead changes from ruler clicks, canvas seeking, keyboard shortcuts, or toolbar controls.
- Expose the badge with an accessible label in the form `Timeline playhead time badge <timecode>`.
- Keep the badge non-interactive so it does not block canvas seeking, scrubbing, clip selection, drag, resize, or drops.
- Keep the playhead line visually dominant over edit-point guide lines.

## Non-Goals

- No schema, project-file, render, Temporal, or asset-generation changes.
- No new playhead snapping behavior.
- No timeline hover preview in this slice.
