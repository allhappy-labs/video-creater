# Generated Output Replacement Kind Gating Design

## Context

Video Creater lets a selected timeline clip become the replacement target for completed AI-generated outputs. The default sample project includes both a generated visual output and an audio timeline clip. When the audio clip is the active replacement target, visual generated outputs should not offer a direct replacement action because that would route video/image media into an audio timeline item.

Palmier-style workflows still need fast generated-output replacement for compatible timeline clips, so the guard must be per-output and preserve replacement for visual clips with visual outputs. Audio outputs should remain eligible for audio clip replacement when the generated output media is audio.

## Requirements

- Completed generated visual outputs must not show `Replace <audio clip> with <visual output>` actions when the active replacement target is an `audio_clip`.
- Compatible generated output replacement actions must remain visible for visual timeline clips.
- The selected generated-source inspector, generated-output list, and Codex selected-source rail must use the same compatibility rule.
- Timeline insertion of generated outputs must remain available; this change only gates replacement of an existing selected clip.

## Design

- Add an optional generated-output replacement compatibility callback to the source media and Codex rail components.
- Default the callback to permissive behavior so isolated component tests and existing callers retain current behavior.
- In `EditorWorkspace`, compute compatibility from the active replacement target and candidate output media:
  - `audio_clip` accepts only `audio` media.
  - visual timeline items accept non-audio media.
  - missing target or missing media is not eligible.
- Keep the project action handler guarded with the same compatibility check so hidden UI cannot be bypassed by stale callbacks.

## Verification

- Add a regression test that selects `Music bed`, selects a completed generated visual output, and verifies the replace action is absent while insertion remains available.
- Add a Codex rail regression test that hides direct replace and queue-and-replace actions when the selected generated media cannot replace the retained target.
- Run the focused editor workspace test file.
- Run lint.
- Browser smoke the sample workspace interaction after implementation.
