# Palmier Generation Reference Slot Shell Flattening

## Intent

Palmier's generation composer keeps first frame, last frame, and reference controls in one compact sheet flow. Video Creater already shows video first/last/reference controls together and hides duplicated `Use selected` shortcuts, but the slot groups still render as nested rounded bordered cards.

## Requirements

- Keep the accessible `First frame slot`, `Last frame slot`, and `Reference slot` groups.
- Remove the rounded border, background shell, and padded card styling from those slot groups.
- Remove the extra rounded bordered wrapper around the video-mode first/last/reference area.
- Preserve filled-slot preview tiles, filenames, metadata, remove buttons, dropdown selection, drag/drop assignment, and queued request payloads.
- Preserve image mode's reference picker and audio mode's lack of visual reference slots.
- Do not change generated asset metadata, fal.ai provider settings, Temporal workflow records, or project schema.

## Testing

- `MediaBin` proves video mode still renders first/last/reference controls together and the slot groups no longer use rounded bordered shell styling.
- Existing request-payload tests continue to prove first-frame, last-frame, and reference IDs are submitted unchanged.
- Browser QA checks the open generation sheet at desktop and narrow widths for readable controls and no horizontal overflow.
