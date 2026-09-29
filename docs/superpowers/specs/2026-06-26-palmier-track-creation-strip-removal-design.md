# Palmier Track Creation Strip Removal Design

## Context

Palmier's timeline ruler starts above the lane labels, and the track column begins directly with compact lane codes such as `V1` and `A1`. Video Creater still renders a persistent `Tracks` label, track-kind selector, and add-track button above the lane labels. That strip makes the timeline look more like a form toolbar than the Palmier reference.

This supersedes the earlier visible placement from `2026-06-26-palmier-track-creation-header-design.md`.

## Requirements

- The timeline ruler header must not render the `Tracks` label.
- The timeline ruler header must not render `Timeline track creation`, `New track kind`, or `Add timeline track` controls.
- The timeline toolbar must continue to omit track creation controls.
- Existing timeline rows, lane labels, track controls, ruler ticks, zoom, drag/drop, and selected clip actions continue to work.
- Track creation remains an internal project action capability for future agent/inspector entry points, but it is not exposed as a persistent timeline-row header control.

## Validation

- Component tests assert the timeline has no visible track creation strip.
- Workspace tests assert the center editor no longer exposes track creation controls in timeline chrome.
- Existing timeline and workspace suites continue to pass after removing the visible strip.
- Browser QA confirms the track column starts directly with lane labels under the ruler.
