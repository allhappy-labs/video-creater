# Palmier Generation Recipe Title Removal Design

## Context

Video Creater's generation composer now shows a compact active recipe summary, but the summary still
renders the visible title `Active generation recipe`. Palmier-style editor chrome keeps metadata
close to the control surface without verbose internal section names. The accessible recipe region is
still useful for tests and assistive technology, but the visible title adds form-like weight inside
the composer.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the redundant visible recipe title while preserving the accessible recipe region and compact
estimate badge.

## Behavior

- The open `Media generation` composer still exposes an `Active generation recipe` region.
- The recipe still shows the estimated credit badge, mode, placement, destination, model, output,
  timing, and reference summary.
- The recipe no longer renders visible `Active generation recipe` text.
- Name, prompt, references, output settings, request payloads, queue behavior, Temporal records,
  fal.ai provider calls, and generated asset metadata remain unchanged.

## Non-Goals

- No recipe data redesign.
- No schema or generated asset metadata changes.
- No mode, model, cost, queue, Temporal, or provider behavior changes.

## Verification

- Update `MediaBin` coverage to assert the accessible recipe region remains while visible
  `Active generation recipe` copy is absent.
- Keep existing active recipe behavior tests passing.
- Browser QA the open generation sheet and confirm the recipe reads as compact metadata rather
  than a titled settings card.
