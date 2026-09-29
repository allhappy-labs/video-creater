# fal.ai Generation Model Defaults

## Problem

The project already records media generation as durable `recordGeneratedAsset` project actions and wraps queued generations in `VideoCreaterGenerateMediaWorkflow` job metadata. The media generation composer still defaults image and video requests to a generic `seedance/seedance-2-fast` model, which does not match the current product requirement to use fal.ai for asset generation.

Palmier-style generation needs queued assets to preserve the exact provider and endpoint that a worker will execute later, so manual UI, Codex proposals, and text-file project state all agree on what was requested.

## Scope

This slice changes the queued generation contract only:

- Image generation requests use provider `fal.ai` and model id `fal-ai/flux/schnell`.
- Video generation requests use provider `fal.ai` and model id `fal-ai/wan-25-preview/text-to-video`.
- Audio generation remains on the existing placeholder model until a dedicated audio provider is selected.
- The generated asset and Temporal job action flow remains unchanged.

## Runtime Policy

Development can keep using mock generation workers. Real end-to-end generation should use the same model ids recorded in project files. Credentials must not be committed to source, test fixtures, or specs; the eventual worker should read the fal.ai key from an environment variable or secure app setting.

## Acceptance

- Media generation UI displays the fal.ai model label for image and video modes.
- Queued video generations record `model: { provider: "fal.ai", id: "fal-ai/wan-25-preview/text-to-video" }`.
- Queued image generations record `model: { provider: "fal.ai", id: "fal-ai/flux/schnell" }`.
- Existing generated asset cards still display historical provider/model labels from project state.
