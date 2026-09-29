# Source Inspector Generated Output Swap Design

## Context

Palmier keeps AI-generated media native to the editing timeline: an editor can inspect a generated clip, see the prompt and references, rerun it, and swap a different generated result into the same timeline position without leaving the project.

Video Creater already supports generated assets, multiple generated outputs, timeline replacement via `replaceTimelineItemWithGeneratedOutput`, and generated output replacement from selected library/Codex sources. The selected timeline clip inspector still lacks a local output picker, so editors must leave the clip context to use an alternate completed output.

## Goal

When a selected timeline source clip belongs to a generated asset with more than one completed output, the Source Inspector details tab should expose the asset outputs and let the editor swap the selected clip to another output in place.

## Behavior

- Show a compact `Outputs` section in the selected timeline clip Source Inspector when:
  - the selected timeline item resolves to a generated asset;
  - that generated asset has at least two outputs;
  - the workspace provides an output replacement callback.
- List each generated output with its filename or media id, technical summary, and media id.
- Mark the output currently used by the selected timeline clip as `Current`.
- For every non-current output, show a `Swap` button.
- On swap, call the existing `replaceTimelineItemWithGeneratedOutput` workspace path with:
  - `itemId`: selected timeline item id;
  - `mediaId`: selected alternate generated output media id.

## Non-Goals

- No new Rust project action or schema.
- No automatic timeline replacement when new variations finish.
- No provider/render workflow changes.
- No replacement affordance for imported media assets in this slice.

## Validation

- Component test: selected generated clip with two outputs shows current and alternate outputs, and clicking `Swap` calls `onReplaceGeneratedOutput` with the alternate media id.
- Workspace test: clicking the selected clip inspector swap button sends `replaceTimelineItemWithGeneratedOutput` for the selected timeline item and alternate media id.
- Existing generated library replacement and insertion tests continue to pass.
