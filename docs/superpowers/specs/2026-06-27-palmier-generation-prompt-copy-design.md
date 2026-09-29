# Palmier Generation Prompt Copy Design

## Context

Video Creater's media generation composer still labels the main creative textarea as
`Generation prompt`. Palmier's editor screenshots present the prompt surface as a direct `Prompt`
field, which makes the composer feel like a compact creative control instead of a verbose settings
form.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the generation textarea use direct prompt-card copy while preserving the accessible control
name and all generation request behavior.

## Behavior

- The open `Media generation` composer shows `Prompt` above the prompt textarea.
- The prompt textarea keeps `Generation prompt` as its accessible label for existing keyboard and
  test affordances.
- The textarea still uses the placeholder `Describe the shot, style, movement, or sound`.
- The composer no longer renders visible `Generation prompt` copy.
- Name, references, output settings, request payloads, queue behavior, Temporal records, fal.ai
  provider calls, and generated asset metadata remain unchanged.

## Non-Goals

- No prompt-card layout redesign.
- No schema or generated asset metadata changes.
- No mode, model, cost, queue, Temporal, or provider behavior changes.

## Verification

- Update `MediaBin` coverage to assert visible `Prompt` copy and absence of visible
  `Generation prompt` copy.
- Keep existing `Generation prompt` accessible-label tests passing.
- Browser QA the open generation sheet and confirm the prompt field reads directly without
  changing composer density.
