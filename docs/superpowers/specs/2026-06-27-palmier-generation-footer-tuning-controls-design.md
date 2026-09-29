# Palmier Generation Footer Tuning Controls Design

## Context

Palmier's generation composer keeps the creative prompt dominant and places model, output settings, credits, and submit affordance in the bottom control strip. Video Creater already has a compact generation sheet, mode tabs, reference tabs, credit balance, history, and a sticky submit footer, but the model/duration/aspect/size controls still sit as a separate row between prompt and submit.

That separate row reads like a settings form and consumes vertical space in the small floating sheet. The controls remain editable and accessible, but visually belong to the same bottom bar as the credit estimate and queue action.

Palmier reference: https://www.palmier.io/docs

## Goal

Move generation tuning controls into the sticky submit footer and remove the standalone tuning-control row from the composer body.

## Requirements

- `Generation submit footer` contains the editable `Generation footer tuning controls` group.
- The tuning controls remain plain `select` fields with the same accessible labels:
  - `Generation model`
  - `Generation duration` for video and audio modes
  - `Generation aspect ratio` for image and video modes
  - `Generation resolution` for image and video modes
- The footer still contains `Generation submit estimate` and the `Queue generation` button.
- The composer body does not render a separate tuning-controls row before the submit footer.
- The footer layout remains compact at desktop sheet width and wraps without text overlap on narrow widths.
- Existing generation request behavior, default fal.ai model choices, credit estimate logic, and queue enablement remain unchanged.
- Do not add a new settings panel, switch, modal, or persistent diagnostics copy.

## Testing

- Update MediaBin tests so the footer is the only location for `Generation footer tuning controls`.
- Add coverage that the footer contains the model, duration, aspect ratio, and resolution fields in video mode.
- Keep existing request-submission tests to prove the same values are submitted.
- Run focused MediaBin tests, typecheck, full Vitest, diff check, and browser QA of the generation sheet at desktop and narrow widths.
