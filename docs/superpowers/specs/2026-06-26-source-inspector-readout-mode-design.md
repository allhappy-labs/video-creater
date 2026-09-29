# Source Inspector Readout Mode Design

## Intent

Palmier keeps the right rail Source view focused on the selected source: file metadata, generation metadata, references, and prompt details. Video Creater currently shows timeline trim, split, opacity, and sequence forms in the Source tab, which makes the default inspector feel like an editing form instead of a source readout.

## Requirements

- Keep the Source tab metadata-first: no timeline trim, split, opacity, audio, or sequence edit controls should render there for selected timeline source clips.
- Preserve manual editing by rendering the editable source-clip controls from the Timeline tab when a timeline source clip is selected.
- Keep generated details, references, prompt, workflow metadata, and source media identity visible from the Source tab.
- Keep existing edit behavior unchanged once the user switches to the Timeline tab.
- Keep selected media source behavior unchanged when no timeline clip is selected.

## Testing

- Add workspace coverage proving the Source tab does not expose `Clip start`, `Source in`, `Split at`, or `Apply clip trim` for a selected source clip.
- Update trim/split workspace tests to switch to the Timeline tab before using manual clip-edit controls.
- Run focused workspace tests, source inspector tests, lint/typecheck, and browser QA for the right rail.
