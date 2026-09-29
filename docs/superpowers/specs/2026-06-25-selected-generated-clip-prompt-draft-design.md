# Selected Generated Clip Prompt Draft

## Context

Palmier lets editors click AI-generated timeline clips, inspect their original prompt and references,
then rerun or tweak the clip from timeline context. Video Creater already exposes generated clip
rerun and replacement actions in the Codex rail, but prompt-tweak actions currently depend on the
global Codex chat prompt. That makes selected-clip iteration less direct and easier to confuse with
unrelated edit requests.

Palmier reference: https://www.palmier.io/docs

## Goal

Give selected generated timeline clips their own compact prompt draft inside the selected clip block.

## Behavior

- Show an editable prompt textarea inside the selected generated clip group when the clip has a
  generated asset id and an original prompt.
- Seed the draft from the selected clip prompt and refresh it when the selected item or prompt
  changes.
- Keep `Rerun same prompt` and `Rerun and replace` tied to the original selected clip prompt.
- Make `Queue variation` and `Queue and replace` use the selected clip prompt draft, trimmed.
- Disable prompt-tweak buttons when the selected clip prompt draft is blank.
- Leave imported clip AI edits and the global Codex chat prompt unchanged.

## Non-Goals

- No new Rust project action, Temporal workflow, fal.ai model, provider behavior, or schema field.
- No generation settings editor inside the selected clip block.
- No variation-set editing in this block.
- No changes to source inspector or media-bin generated source composer behavior.

## Verification

- AgentPanel test: selected generated timeline clips expose an editable prompt draft seeded from the
  original prompt, use it for variation and replacement variation actions, and disable those actions
  when the draft is blank.
- AgentPanel test: queue transcript entries record the selected clip prompt draft for selected
  generated timeline clip variations.
- EditorWorkspace test: the Codex rail selected generated clip replacement variation records the
  draft prompt in the Temporal start request and generated asset action.
- Run focused AgentPanel and EditorWorkspace tests, TypeScript checks, whitespace checks, and the
  secret-fragment scan.
