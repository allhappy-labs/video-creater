# Generation Use Selected Reference Design

## Context

Palmier's generation composer keeps first frame, last frame, and reference media directly beside the prompt so editors can anchor a generation from the current visual source without leaving the timeline/library flow. Video Creater already supports first/last/reference slots, drag/drop, defaulting from the selected visual asset when the composer opens, and text-file-backed generation requests. The missing interaction is an explicit way to apply the currently selected visual media to any reference slot after the composer is open.

## Goal

Add compact `Use selected` actions to generation reference slots so an editor can assign the current selected visual media as first frame, last frame, or reference without hunting through the select menu.

## Behavior

- Each visual generation reference slot shows `Use selected` when the selected media is visual and is not already assigned to that slot.
- Clicking the action assigns `selectedMediaId` to that slot.
- Audio selections and no selection hide the action.
- Existing slot select menus, drag/drop, preview thumbnails, remove actions, and generation request payloads stay unchanged.
- Video first/last mode can use the same selected media for first and last frame when the user explicitly chooses that.
- Image and video reference mode use the same action for the reference slot.

## Non-Goals

- No multi-reference payloads.
- No timeline selection changes.
- No new Temporal workflow contract or project action type.
- No provider-specific fal.ai request changes.

## Verification

- `MediaBin` tests prove `Use selected` appears for an empty last-frame slot and queues the selected media as `lastFrameMediaId`.
- `MediaBin` tests prove `Use selected` is hidden for audio selections.
- Existing generation composer tests keep covering prompt, name, model, settings, drag/drop, and placement behavior.
