# Source Inspector Audio Fade Edit Design

## Context

Video Creater now renders `properties.fadeOutSeconds` as a visible audio timeline ramp. That makes fade metadata inspectable in the timeline, but editors and agents still need a validated way to update the value without hand-editing JSON. Palmier's timeline makes clip adjustments feel native, and its agent model expects trim, split, reorder, and adjustment operations to flow through durable project state.

## Goal

Add a compact Source Inspector control and validated project action for editing an audio clip's fade-out duration.

## Behavior

- When the selected timeline item is an `audio_clip`, Source Inspector shows `Fade out` beside the existing trim fields.
- The input defaults to the current positive `fadeOutSeconds` value, or blank when absent.
- Applying the fade sends a dedicated `updateAudioFadeOut` project action.
- The action accepts a finite non-negative value, stores positive values as `properties.fadeOutSeconds`, and removes the property for zero.
- The action rejects missing items, locked tracks, non-audio items, non-finite values, and negative values.
- Existing clip trim, split, reorder, waveform, and fade-ramp rendering remain unchanged.

## Non-Goals

- No audio renderer or export mix semantics.
- No fade-in support.
- No draggable fade handles.
- No generic arbitrary-property mutation action.

## Verification

- `SourceClipInspector` tests prove audio clips show the fade input and submit a fade update.
- `EditorWorkspace` tests prove applying the inspector fade sends `updateAudioFadeOut` to the split-project action command.
- Rust reducer tests prove the action stores positive fades, removes zero fades, and rejects non-audio items.
- Existing timeline and source inspector tests continue to pass.
