# Palmier Track Lock Toggle Removal Design

## Context

Palmier's timeline track headers show compact track identity plus link and media-state controls. Video Creater also renders a lock/unlock button on every track row, which makes the timeline header busier than the reference editor. Track lock state still matters for validation and agent workflows, but it does not need to be a persistent visible row toggle.

## Requirements

- Timeline track header controls render only:
  - disabled linked-track affordance,
  - visible/hidden control for visual tracks,
  - audible/muted control for audio tracks.
- Track lock/unlock buttons are not rendered in the timeline track header.
- Existing lock state remains functional:
  - locked tracks still disable item mutation and resize handles,
  - locked-track status remains visible in agent/inspector contexts that already expose it,
  - Codex/project actions can still set track lock state.
- Track media-state toggles continue to work from the timeline row controls.

## Validation

- Component tests assert track controls omit `track locked` and `track unlocked` buttons.
- Editor workspace tests assert timeline rows no longer expose lock buttons while the existing Codex lock action still submits `setTrackLocked`.
- Existing locked-track behavior, media-state toggles, and timeline mutation tests continue to pass.
- Browser QA confirms track headers visually match Palmier's link plus media-state pattern.
