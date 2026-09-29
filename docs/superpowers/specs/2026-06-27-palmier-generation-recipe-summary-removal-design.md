# Palmier Generation Recipe Summary Removal

## Goal

Remove the duplicate `Active generation recipe` summary from the media generation sheet. Palmier presents generation intent through the selected slots, prompt field, compact model/settings strip, and submit footer, not a second read-only summary table.

## Design

- Remove the `Active generation recipe` region from `MediaBin`.
- Keep visible first-frame, last-frame, and reference slots as the source of truth for visual references.
- Keep the prompt field, model selector, duration, aspect ratio, resolution, estimate, readiness, and queue action in the existing footer controls.
- Keep placement behavior unchanged: library remains the default, timeline and replacement placement still affect queued requests and existing placement note copy.
- Keep all `onGenerateMedia`, generated asset, folder target, and Temporal workflow request payloads unchanged.
- Do not add another summary card or replacement status table.

## Testing

- `MediaBin` verifies the generation sheet no longer renders `Active generation recipe`.
- Existing tests move duplicated recipe assertions to the visible slots and footer tuning controls.
- `EditorWorkspace` verifies media-panel generation still opens as a library draft without exposing a recipe summary.
- Browser QA confirms the live generation sheet has one set of model/settings controls and no duplicate active-recipe text.
