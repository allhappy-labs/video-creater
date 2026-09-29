# Manual Timeline Track Creation Design

## Context

Palmier shows multi-lane timelines where generated clips, imported footage, captions, and audio can be arranged without leaving the editor. Video Creater can now move items across compatible tracks, but users and agents do not have a validated project action for creating additional tracks when the existing lanes are not enough.

## Goal

Add a text-file-backed `createTrack` workflow and a compact timeline toolbar control for adding another timeline lane of a chosen kind.

## Behavior

- The timeline toolbar exposes an icon-only `Add timeline track` control near the existing manual edit tools.
- Activating the control creates a new unlocked, enabled track after the last existing track of the same kind.
- The default kind is `video`, because extra video lanes unlock the cross-track move and replacement workflow most directly.
- Track ids are stable and human-readable: `track-video-2`, `track-audio-2`, and so on.
- Track names follow the visible editor convention: `Video 2`, `Audio 2`, `Captions 2`, `Overlays 2`, or `HyperFrames 2`.
- The action persists through Rust `ProjectAction::CreateTrack` for split and embedded projects, so `timeline.json` remains the source of truth.
- Rust rejects duplicate track ids, empty names, and empty ids.

## UI Scope

This slice adds the project action and one toolbar button for a video track. A future slice can expand this into a small kind picker once multiple-lane editing is proven end to end.

## Implementation Notes

- Add `CreateTrack { track: TimelineTrack, afterTrackId?: string }` to TypeScript and Rust project actions.
- Insert the new track after `afterTrackId` when provided; otherwise append it.
- Keep `items` empty on newly created tracks.
- Generate the new track client-side from the current timeline so the user sees the same ids the agent can use later.
- Add the action to the Codex app-server allowed action list so agent proposals can create tracks explicitly.

## Verification

- Rust tests prove `createTrack` inserts a track after a specific track and rejects duplicate ids.
- Timeline editor tests prove the toolbar exposes an accessible add-track command.
- Workspace tests prove clicking the command submits a split-project `createTrack` action with a stable track id and name.
- Browser QA checks that the added control fits in desktop and narrow timeline toolbars.
