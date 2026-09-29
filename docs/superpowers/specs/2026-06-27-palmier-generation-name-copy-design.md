# Palmier Generation Name Copy Design

## Context

Video Creater's media generation composer still labels the optional output name field as
`Generation name`. Palmier's prompt card treats the name as a lightweight optional field, which
keeps the composer feeling closer to an editor prompt surface and less like a long settings form.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the generation name field read as optional editor copy while preserving the accessible control
name and all generated asset naming behavior.

## Behavior

- The open `Media generation` composer shows `Name (optional)` above the name input.
- The name input keeps `Generation name` as its accessible label for existing keyboard and test
  affordances.
- The input still uses the placeholder `Name generated asset`.
- The composer no longer renders visible `Generation name` copy.
- Prompt text, reference slots, output settings, request payloads, queue behavior, Temporal
  records, fal.ai provider calls, and generated asset metadata remain unchanged.

## Non-Goals

- No schema or generated asset metadata changes.
- No prompt-card redesign.
- No mode, model, cost, queue, Temporal, or provider behavior changes.

## Verification

- Update `MediaBin` coverage to assert the visible `Name (optional)` copy and absence of visible
  `Generation name` copy.
- Keep existing `Generation name` accessible-label tests passing.
- Browser QA the open generation sheet and confirm the name field reads as optional without
  changing composer density.
