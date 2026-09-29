# Mock Generation Provider Gating Design

## Context

Palmier documents image, video, and audio generation from the media panel. Video Creater now exposes those modes, but the local mock completion worker only knows how to synthesize fal.ai image and video outputs. Audio generation currently uses a placeholder ElevenLabs model, so offering `Complete mock` on audio jobs creates a broken action path.

## Goal

Only expose mock completion controls for generated assets whose provider/model pair is supported by the mock worker.

## Behavior

- Queued and running fal.ai FLUX image jobs can show `Complete mock`.
- Queued and running fal.ai Wan video jobs can show `Complete mock`.
- Queued and running unsupported providers, including the placeholder audio provider, do not show mock completion controls.
- Completed, failed, and already-output assets keep the existing behavior.
- This does not add audio mock generation or change generated-asset schema.

## Verification

- Media Bin tests prove unsupported audio placeholder jobs hide `Complete mock`.
- Existing Media Bin tests prove fal-backed queued variation jobs can still complete through the mock action.
