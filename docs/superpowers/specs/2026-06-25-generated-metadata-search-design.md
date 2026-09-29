# Generated Metadata Search

## Context

Palmier's media panel treats generated assets as project-library objects that can be generated,
organized, regenerated, and swapped without leaving the editor. Video Creater now renders generated
card metadata such as folder destination and replacement/timeline target badges, but the project
media search only matches prompt, model, references, lineage, and output paths.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the project media search find generated assets by the same placement and organization metadata
that is visible on their cards.

## Behavior

- Searching a generated asset's visible destination folder label keeps that generated card visible.
- Nested folder labels use the same path text as the media bin, such as
  `Generated selects / Scene A`.
- Searching visible placement badges keeps matching generated cards visible:
  - `Timeline target`
  - `Replacement target`
  - `Library`
- Existing prompt, model, reference, lineage, and output-path search behavior stays unchanged.

## Non-Goals

- No search indexing service or fuzzy matching.
- No backend, project schema, Temporal workflow, or fal.ai provider change.
- No automatic navigation to folders or timeline targets.

## Verification

- Media Bin test: generated assets remain visible when searching by a nested destination folder.
- Media Bin test: generated assets remain visible when searching by replacement target metadata.
- Existing Media Bin search and generated-card tests continue passing.
