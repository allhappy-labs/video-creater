# Codex Selected Track State Intent

## Context

Palmier-style agents can adjust timeline structure while seeing full project context. Video
Creater already exposes manual track lock and visibility buttons in the timeline, and those
buttons write validated `setTrackLocked` and `setTrackEnabled` project actions through Rust.
The Codex selected-clip context shows the current track name, locked state, and visible or
audible state, but explicit composer prompts such as `Lock the selected track` or
`Hide the selected track` still fall back to a broad edit request.

## Goal

Let Codex apply selected timeline track lock and enabled-state changes from explicit prompts,
without inventing new project actions and without bypassing Rust-owned split-project
validation.

## Behavior

- The route is only available when a selected timeline clip has a non-empty `trackId`.
- Lock prompts include `lock` plus `selected track`, `current track`, `this track`, or
  `track`.
- Unlock prompts include `unlock` plus the same track reference terms.
- Hide prompts include `hide` or `disable` plus a track reference.
- Show prompts include `show`, `enable`, or `unhide` plus a track reference.
- Audio-track prompts may also use `mute` for disabled and `unmute` for enabled.
- Pressing the primary composer action calls the selected-track callback with the selected
  clip's `trackId` and the requested boolean state.
- The composer button label reflects the action: `Lock selected track`, `Unlock selected
  track`, `Hide selected track`, `Show selected track`, `Mute selected track`, or
  `Unmute selected track`.
- The transcript records a `project_action` tool row naming the track-state change.

## Non-Goals

- No freeform track selection by name in this slice.
- No new Rust project action shape.
- No optimistic local-only state; `EditorWorkspace` continues to call `applyProjectAction`.
- No broad LLM edit request for explicit selected-track state changes.

## Tests

- `AgentPanel` proves explicit lock and hide prompts route through selected-track callbacks,
  hide `Generate edit`, and record project-action transcript rows.
- `EditorWorkspace` proves explicit selected-track lock and visibility prompts write
  `setTrackLocked` and `setTrackEnabled` split-project actions for the selected clip's track.
