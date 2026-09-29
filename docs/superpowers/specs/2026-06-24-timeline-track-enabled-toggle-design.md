# Timeline Track Enabled Toggle Design

## Context

Palmier's timeline track headers expose compact link, eye, and speaker controls. Video Creater now has real lock toggles, but the eye/speaker controls are still read-only. That leaves a visible manual editing affordance without project-state semantics, and agents cannot inspect or change whether a track is intended to contribute visual or audio output.

## Goal

Add a persisted per-track enabled state:

- Visual tracks use the state as visible or hidden.
- Audio tracks use the same state as audible or muted.
- The state is stored in timeline project files and changed through validated project actions.

## Behavior

- `TimelineTrack.enabled` defaults to `true` for existing projects that do not yet include the field.
- `TimelineEditor` accepts `onToggleTrackEnabled(trackId, enabled)`.
- The media-state button is enabled only when that callback is supplied.
- Clicking a visible visual track requests `enabled: false` and relabels it as hidden when the project updates.
- Clicking a hidden visual track requests `enabled: true`.
- Clicking an audible audio track requests `enabled: false` and relabels it as muted when the project updates.
- Clicking a muted audio track requests `enabled: true`.
- Disabled tracks remain editable but their timeline items render visually subdued so the state is inspectable in the timeline.
- Link controls remain read-only until linked-track editing has a validated model.
- Rust applies `setTrackEnabled` only to existing timeline tracks and rejects unknown track ids with `TrackNotFound`.

## Verification

- Component tests prove media-state buttons call the toggle callback with track id and next enabled state.
- Workspace tests prove split projects dispatch `setTrackEnabled` and reflect the returned hidden/muted state.
- Rust tests prove `SetTrackEnabled` updates an existing track and rejects a missing track.
- Serialization keeps older timeline tracks loadable through a default enabled value.
