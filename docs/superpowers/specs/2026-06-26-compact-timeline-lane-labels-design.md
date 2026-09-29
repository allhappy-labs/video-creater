# Compact Timeline Lane Labels Design

## Context

Palmier's timeline uses compact lane identities such as `V1`, `A1`, and `A2`, leaving more room for
clips while keeping track controls close to each lane. Video Creater currently renders full track
names in the lane header column, which makes long labels like `HyperFrames` truncate and gives the
timeline less of a professional editor shape.

Palmier reference: https://www.palmier.io/docs

## Goal

Show compact editor-style lane codes in the timeline track header while preserving full track names
for accessible labels, tooltips, drag/drop feedback, and existing track controls.

## Behavior

- Timeline track headers show a compact lane code derived from track kind and order:
  - video: `V1`, `V2`, ...
  - audio: `A1`, `A2`, ...
  - captions: `C1`, `C2`, ...
  - overlays: `O1`, `O2`, ...
  - HyperFrames: `H1`, `H2`, ...
- The existing track name remains available through the header title, drag target aria label when
  drag feedback is active, and track control labels.
- Track controls keep their current link, visibility/mute, and lock behavior.
- Timeline clip labels, generated AI badges, workflow status, drag/drop behavior, and selection
  behavior remain unchanged.

## Non-Goals

- No timeline schema changes.
- No track renaming or persistent lane-code fields.
- No changes to project actions, Temporal workflows, render plans, or media preview rendering.

## Verification

- Add a `TimelineEditor` test proving lane headers render compact codes for video, audio, captions,
  overlays, and HyperFrames while keeping the full track name inspectable.
- Existing timeline track-control and drag/drop tests continue to pass.
- Browser-smoke the sample editor timeline and confirm the left lane column is compact with no label
  overlap.
