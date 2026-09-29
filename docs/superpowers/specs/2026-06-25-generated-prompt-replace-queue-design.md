# Generated Prompt Replace Queue

## Context

Palmier keeps generated clip iteration native to the timeline: users can rerun or tweak generated
clip prompts and swap the result back onto the timeline without leaving the project. Video Creater
already supports completed generated output replacement and same-prompt replacement reruns. The
remaining gap is prompt-tweak replacement at queue time: the Source Inspector AI Edit panel can
queue a variation, but that queued variation does not explicitly target the selected timeline clip
for replacement.

Palmier reference: https://www.palmier.io/docs

## Goal

Let a selected generated timeline clip queue a tweaked-prompt variation that will replace that same
timeline item when the generated output completes.

## Behavior

- In Source Inspector's generated `AI Edit` tab, keep the existing `Queue variation` action for
  making another output.
- When the inspector is editing a selected generated timeline item, show `Queue and replace selected
  clip`.
- The replacement action uses the same variation prompt field and generation context as
  `Queue variation`.
- The queued generated asset records `placementIntent: "replace:<timeline-item-id>"`.
- Existing Temporal/fal generation completion code continues to resolve that placement intent and
  produce the replacement action after a generated output is available.
- The action is disabled when the prompt is blank.

## Non-Goals

- No new canonical project mutation path.
- No automatic replacement before generation completion.
- No variation-set replacement mode in this slice; replacement sets need a separate selection model
  for choosing the winning output.
- No generated media model/settings changes.

## Verification

- SourceClipInspector test: a tweaked prompt calls the replacement variation callback with asset id,
  prompt, and selected timeline item id.
- EditorWorkspace test: the queued generated asset and Temporal start request carry
  `placementIntent: "replace:<timeline-item-id>"`.
- Run relevant SourceClipInspector and EditorWorkspace suites, TypeScript checks, whitespace checks,
  and the secret-fragment scan.
