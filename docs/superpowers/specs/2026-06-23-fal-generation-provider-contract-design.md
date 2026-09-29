# fal.ai Generation Provider Contract

## Problem

Video Creater now records new image and video generations with fal.ai model ids, but the Rust workflow side does not yet have a concrete provider boundary that can turn a queued generated asset into a fal queue request. Temporal activities need that boundary before they can run real generation or a development mock against the same project-file contract.

## Scope

Add a Rust generation provider contract for queued `GeneratedAsset` records:

- `fal.ai/fal-ai/flux/schnell` maps to the fal text-to-image endpoint.
- `fal.ai/fal-ai/wan-25-preview/text-to-video` maps to the fal text-to-video endpoint.
- Unsupported provider/model pairs fail before any network call.
- The provider contract builds request payloads only; Temporal worker execution, polling, download/import, and completion actions remain future slices.

## Request Mapping

FLUX image requests:

- Endpoint: `fal-ai/flux/schnell`
- Input fields: `prompt`, `image_size`, `num_images`, `output_format`, `enable_safety_checker`
- Use explicit `{ width, height }` when project settings include dimensions.
- Use PNG output for editor-importable generated stills.

Wan text-to-video requests:

- Endpoint: `fal-ai/wan-25-preview/text-to-video`
- Input fields: `prompt`, `aspect_ratio`, `resolution`, `duration`, `enable_prompt_expansion`, `enable_safety_checker`
- Aspect ratio defaults to `16:9` and accepts only `16:9`, `9:16`, or `1:1`.
- Resolution is normalized to fal's `480p`, `720p`, or `1080p` tiers from requested dimensions.
- Duration is normalized to fal's supported `5` or `10` seconds. Requests up to 5 seconds use `5`; longer requests use `10`.

## Credential Policy

The request builder must not accept, persist, or log API keys. Real execution should read `FAL_KEY` from worker runtime configuration or a secure app setting. Development and unit tests use the same request builder with a mock executor.

## Acceptance

- Rust tests prove FLUX and Wan generated assets produce the expected fal queue request.
- Rust tests prove unsupported provider/model pairs return a typed error.
- The repository contains no fal.ai secret material.
