# Palmier Generated Source Details Merge

## Context

The source inspector currently shows generated media context in two adjacent places:

- a top-level `Generated recipe summary` group under the generated source header;
- a `Generated details` group with file details, references, prompt, and AI edit actions.

This keeps useful metadata visible, but it creates a second properties block in the right rail. Palmier-style inspectors favor one compact source/details panel rather than repeated metadata sections.

## Goal

Merge generated recipe metadata into the existing generated file details and remove the separate `Generated recipe summary` group.

## UX Requirements

- Generated source model, aspect ratio, resolution, duration, status, file type, frame rate, and path remain visible.
- The source inspector exposes only one generated metadata/details locus: `Generated details`.
- `Generated recipe summary` is no longer rendered in the source inspector.
- The `Generated AI edit` actions remain inline below generated details.
- Existing generated variation, replacement, upscale, insert, reveal, and copy-prompt actions keep their behavior.

## Implementation Requirements

- Add `Model`, `Aspect`, and `Status` rows to `Generated file details`.
- Keep existing file detail rows and generated references/prompt structure.
- Remove the `renderGeneratedRecipeSummary` source-readout block from generated sources.
- Do not change project data schemas, generated asset records, or timeline actions.

## Acceptance Criteria

- Tests verify `Generated recipe summary` is absent for generated source clips and generated output selections.
- Tests verify `Generated file details` contains model, aspect, resolution, duration, status, type, frame rate, and path.
- Tests verify generated AI edit remains visible and usable without tabs.
- Browser QA confirms selecting a generated timeline clip shows a single generated details section in the right rail.
