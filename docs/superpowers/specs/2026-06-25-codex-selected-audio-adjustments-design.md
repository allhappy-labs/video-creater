# Codex Selected Audio Adjustments

## Context

Palmier documents agent-controlled editing where connected agents can trim, split, reorder, and
adjust timeline clips while users keep manual editor control. Video Creater already supports
timeline-native audio fade and gain edits through Source Inspector, with validated
`updateAudioFadeOut` and `updateAudioVolume` split-project actions. The Codex rail can see the
selected timeline clip and operate on trim, split, reorder, and generated-clip reruns, but selected
audio clips still lack nearby agent-facing adjustment controls.

## Goal

Expose compact fade-out and volume controls in the Codex selected timeline clip block for selected
audio clips, using the same validated project actions as Source Inspector.

## Behavior

- When the selected timeline clip kind is `audio_clip`, the Codex rail shows an `Audio` adjustment
  group inside `Selected timeline clip`.
- The fade field is labelled `Fade out`, defaults to the current positive
  `properties.fadeOutSeconds` value, and applies a non-negative value through
  `updateAudioFadeOut`.
- A blank fade field applies `0`, which removes `properties.fadeOutSeconds` through the existing
  reducer behavior.
- The volume field is labelled `Volume dB`, defaults to the current numeric
  `properties.volumeDb` value, and applies values from `-60` through `24` through
  `updateAudioVolume`.
- A blank volume field applies `null`, which resets the clip to unity gain through the existing
  reducer behavior.
- Locked tracks show the controls but disable the apply buttons and emit no project actions.
- Non-audio selected clips do not show the audio adjustment group.
- The implementation adds no new project schema, renderer, Temporal, or fal.ai behavior.

## Verification

- `AgentPanel` tests prove selected audio clips show fade and volume controls and emit the expected
  callback payloads.
- `AgentPanel` tests prove locked selected audio clips disable fade and volume actions.
- `EditorWorkspace` tests prove Codex audio fade and volume actions call
  `apply_project_action_to_split_project_folder` with `updateAudioFadeOut` and `updateAudioVolume`.
- Existing Source Inspector, timeline, and selected-clip Codex tests continue passing.

## Non-Goals

- No keyframed audio automation.
- No fade-in support.
- No audio render mix changes.
- No arbitrary property editor in Codex.
- No natural-language parsing for audio adjustment requests.
