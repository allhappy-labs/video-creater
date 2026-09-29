# Generated Asset Recency Order Design

## Context

Palmier's generation workflow keeps recent AI outputs close to the editor loop: users generate,
rerun, tweak, inspect, and swap clips without leaving the project. Video Creater already records
`createdAt` on generated assets and exposes both a Media Bin generation history drawer and an
`AI generations` list. Those surfaces currently preserve the incoming array order, which can become
oldest-first when project files are hand-edited, merged, or loaded from split text files.

## Goal

Show generated assets newest-first wherever the Media Bin presents recent generation history or AI
generation cards.

## Behavior

- Generation history shows the four most recently created generated assets.
- The `AI generations` list displays visible generated assets newest-first.
- Sorting uses `createdAt` descending.
- Assets with invalid or missing timestamps fall behind valid timestamps but keep their original
  relative order.
- Search filtering still applies before display and does not change which assets match.

## Non-Goals

- No schema changes to generated assets or workflow jobs.
- No backend migration, Temporal workflow change, provider integration, or persistence rewrite.
- No new user-facing sort control for generated assets.
- No mutation of the `generatedAssets` prop.

## Validation

- Media Bin tests prove generation history and `AI generations` cards use newest-first ordering.
- Existing generation history reuse, workflow status, lineage, and generated output action tests
  continue to pass.
- Browser QA checks that the generated section and history drawer still fit in the source library.
