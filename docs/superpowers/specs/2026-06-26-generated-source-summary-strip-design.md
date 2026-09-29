# Generated Source Summary Strip Design

## Context

Palmier's source inspector makes generated media easy to understand at a glance: model, aspect,
resolution, duration, and status sit close to the source title before deeper file, reference, and
prompt details. Video Creater already stores and displays this data, but it is split across detailed
sections, so the right rail takes more scanning than it should.

Palmier reference: https://www.palmier.io/docs

## Goal

Add a compact generated-source summary strip at the top of generated details so the active AI media
recipe is immediately visible.

## Behavior

- Generated details render a `Generated recipe summary` group before action buttons and detail
  sections.
- The group shows model, aspect ratio, resolution, duration, and generated status.
- The group uses compact badge-like cells that wrap inside the right rail without hiding existing
  actions.
- Existing generated details, references, prompt copy, outputs, AI edit actions, and callback
  payloads remain unchanged.

## Non-Goals

- No generated asset schema, workflow, Temporal, or fal.ai changes.
- No new model selection, prompt editing, or output replacement behavior.
- No change to imported media inspector details.

## Verification

- Add a `SourceClipInspector` test proving the summary group renders model, aspect ratio,
  resolution, duration, and status before the full details.
- Existing generated source details, reference tile, prompt copy, and AI edit tests continue to pass.
- Browser-smoke the right rail and confirm the summary strip fits without overlapping the reference
  tiles or prompt content.
