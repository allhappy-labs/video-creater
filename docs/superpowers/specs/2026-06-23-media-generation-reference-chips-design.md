# Media Generation Reference Chips

## Context

Palmier's media-generation workflow makes first frame, last frame, and reference media visibly present in the composer. Video Creater already supports those references through dropdowns, but the current composer hides the selected assets inside small select fields. That makes the generation setup harder to scan, especially when using the selected media as a first frame and style reference.

## Goal

Make generation references visible and editable in the media panel composer without changing the underlying generated-asset project action.

## Behavior

- When first frame, last frame, or reference media is selected, the composer shows a compact chip for each selected role.
- Each chip includes:
  - role label: `First frame`, `Last frame`, or `Reference`
  - selected filename
  - concise media metadata when available
  - a remove button with an accessible label
- Removing a chip clears the matching selection.
- Audio generation hides these visual reference chips because audio requests do not use visual references.
- The existing dropdowns remain available for precise selection.

## Non-Goals

- No drag-and-drop reference assignment in this slice.
- No multi-reference list beyond the existing single reference media selector.
- No schema or backend changes.
- No generated output preview thumbnails in the composer yet.

## Implementation Notes

Add a small helper inside `MediaBin` to resolve a selected reference id into a chip. Keep this component local to the media bin because it only reflects composer state. Use existing media metadata helpers and shadcn buttons. The remove affordance should use a familiar close icon and must not resize the composer unexpectedly.

## Tests

- Opening the generation composer with selected visual media shows first-frame and reference chips for that media.
- Changing last-frame/reference selections updates the visible chips.
- Clicking a chip remove button clears that selection from the eventual generation request.
- Switching to audio hides the visual reference chips and submits no visual references.
