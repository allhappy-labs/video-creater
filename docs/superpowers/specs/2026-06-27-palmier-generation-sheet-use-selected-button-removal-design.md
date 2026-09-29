# Palmier Generation Sheet Use Selected Button Removal Design

## Intent

Palmier keeps media generation references in a compact sheet: first frame, last frame, references, prompt, format, model, and a single send action. Video Creater already exposes explicit selectors and drag/drop zones for first, last, and reference media, but it also repeats a `Use selected` shortcut inside empty slots. That extra button layer makes the sheet feel busier without adding a separate workflow.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Remove visible `Use selected` buttons from empty first-frame, last-frame, and reference controls in the `Media generation` sheet.
- Preserve first-frame auto-seeding from the selected visual media when the sheet opens.
- Preserve explicit first-frame, last-frame, and reference selection through the existing dropdowns.
- Preserve drag/drop assignment for first-frame, last-frame, and reference slots.
- Preserve compact filled-slot preview tiles and their remove buttons.
- Keep generation request payloads, Temporal workflow handoff, fal.ai provider calls, and generated asset metadata unchanged.

## Testing

- Update `MediaBin` coverage to prove visual selections no longer render any `Use selected media as ...` buttons.
- Keep tests proving explicit dropdown selection submits the same reference payload.
- Keep tests proving filled first/last/reference slots stay compact and removable.
- Run the focused media-bin suite, editor-workspace generation coverage, lint, full tests, and browser QA for the generation sheet.

## Self-Review

- Scope is limited to visible generation-sheet slot chrome and tests around the preserved selectors.
- The removal does not change project files, timeline files, media metadata, provider schemas, or Temporal queue behavior.
- Empty slots remain operable through select controls and drag/drop, so manual editing stays available without duplicated shortcut buttons.
