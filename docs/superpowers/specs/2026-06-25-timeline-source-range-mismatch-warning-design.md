# Timeline Source Range Mismatch Warning

## Context

Video Creater projects are text-editable by humans and agents. The source inspector and split-project validator already reject clips where `sourceOut - sourceIn` does not match `durationSeconds`, because Video Creater has no retiming model for mismatched source spans. The timeline currently shows the source range text but does not mark invalid source-range duration data directly on the clip.

## Goal

Surface source range duration mismatches directly on timeline clips so invalid project data is visible while editing.

## Requirements

- Media-backed clips with complete numeric `sourceIn` and `sourceOut` compare `sourceOut - sourceIn` to `durationSeconds`.
- If the absolute difference is greater than `0.01s`, render a compact warning badge on the clip.
- The badge has an accessible label naming the clip, source span duration, and clip duration.
- Valid complete source ranges do not render the warning.
- Clips without complete source ranges do not render the warning.
- Text-only timeline items do not render the warning even if they contain source-like properties.
- Existing source range labels, source boundary markers, selection, drag, resize, filmstrip, and waveform behavior remain unchanged.

## Non-Goals

- Automatic repair of invalid ranges.
- Retiming or speed controls.
- Project schema changes.
- Replacing existing source inspector or Rust split-project validation.

## Test Plan

- Add a TimelineEditor component test for a media-backed source range mismatch warning.
- Add a TimelineEditor component test proving a valid source range has no warning.
- Run focused timeline editor tests, full frontend tests, lint, build, browser QA, whitespace checks, placeholder scan, and secret scan.
