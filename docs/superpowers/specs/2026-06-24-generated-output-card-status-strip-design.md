# Generated Output Card Status Strip

## Context

Palmier keeps generated media easy to scan by carrying generation cues on AI media thumbnails and timeline-adjacent generated output cards. Video Creater now shows generation status strips on generated media tiles and list rows in the project library, but the same generated clip preview inside the `AI generations` section still looks like a plain thumbnail.

## Goal

Show the same compact generation status cue on generated output preview cards in the `AI generations` section so agents and manual editors can recognize completed, running, and failed outputs without switching back to the project library.

## Behavior

- Generated output cards with a resolved media thumbnail show a thin bottom status strip.
- The strip uses the owning generated asset status.
- Completed outputs use a cyan strip; queued/running outputs use amber; failed outputs use destructive red.
- The output-card strip exposes an accessible label in the form `Generation output status {status}`.
- Pending generated assets without resolved output media keep their existing pending text and do not render an output-card strip.
- Project library generated media tile and row strips keep their existing `Generation status {status}` labels.

## Non-Goals

- No generated asset schema changes.
- No workflow status or provider queue changes.
- No timeline placement behavior changes.
- No new generated output actions.

## Tests

- Add a `MediaBin` test assertion that a completed generated output card exposes `Generation output status completed`.
- Keep the existing project-library status strip assertions intact.
