# Generation Model Selector Design

## Context

Palmier's docs describe generation as choosing a model, writing a prompt, and tuning output settings
from the media panel. Video Creater already records generation model provenance and uses the required
fal.ai defaults for image and video, but the composer only exposes the model as a passive footer
summary. That makes model choice feel less like an editor-controlled setup step and hides the
provider/model contract until submission.

## Goal

Expose the active generation model as an explicit selector in the Media Bin generation composer while
preserving the existing default model payloads.

## Behavior

- The composer shows a compact `Generation model` select near the output settings.
- Image mode selects `fal-ai/flux/schnell`.
- Video mode selects `fal-ai/wan-25-preview/text-to-video`.
- Audio mode selects the existing placeholder `elevenlabs/music-v1`.
- Switching modes keeps prompt, name, references, placement, and output settings intact while
  presenting the model options for the active mode.
- Queueing generation uses the selected model object and keeps writing model provenance to project
  actions and Temporal start requests.

## Non-Goals

- No new provider implementation, real fal.ai network call, credential storage, or Temporal workflow
  change.
- No unsupported model ids in the selectable list.
- No multi-model ranking, pricing table, or account-credit integration.

## Validation

- Media Bin tests prove the `Generation model` selector appears and changes with mode.
- Existing Media Bin and Editor Workspace tests keep proving queued image and video requests use the
  required fal.ai model ids.
- Browser QA checks the model selector is visible in the live generation composer and does not crowd
  the prompt or submit footer.
