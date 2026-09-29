# Source Preview Step Keyboard Shortcuts

## Context

Palmier treats source footage as a first-class editing surface: users can open source media from the timeline, inspect generation provenance, and continue editing without leaving the project. Video Creater now exposes source preview tabs, real media playback, and transport step buttons, but the stepping workflow still requires mouse interaction.

## Goal

Add keyboard shortcuts for source preview stepping so manual reviewers and agents can align on predictable source positions before creating timeline edits.

## Requirements

- `ArrowLeft` steps the active source preview backward.
- `ArrowRight` steps the active source preview forward.
- Video and generated video step by one frame using the source FPS label, falling back to 24 fps.
- Audio steps by one second.
- Image sources and sources without playable preview URLs do not step.
- Shortcuts are ignored when focus is inside editable text controls or when modifier keys are held.
- Shortcuts clamp to valid media bounds and prevent browser defaults only when a step is actually handled.
- Transport tooltips expose the matching keyboard shortcuts.

## Non-Goals

- Timeline playhead stepping.
- Source range mark-in or mark-out changes.
- Scrubber drag interaction.
- Persistent source preview current-time serialization.

## Test Plan

- Add component tests for `ArrowLeft` and `ArrowRight` source stepping.
- Add a component test proving editable focus does not trigger stepping.
- Run the focused preview panel test file.
- Run full frontend tests, lint, build, whitespace checks, placeholder scan, secret scan, and browser QA.
