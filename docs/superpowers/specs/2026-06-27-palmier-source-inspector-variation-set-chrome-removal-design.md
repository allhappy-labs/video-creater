# Palmier Source Inspector Variation Set Chrome Removal Design

## Intent

Palmier's generated-media editing surface keeps the immediate editor control focused on one prompt/reference generation at a time. Bulk variation exploration exists as an assistant workflow: the user asks for several directions and the generated assets appear in the library while the timeline stays available. Video Creater currently exposes a permanent `Variation set` block inside `Generated AI edit`, with four name/prompt rows and a `Queue variation set` button. That makes the source inspector feel like a batch form instead of a compact editor inspector.

## Requirements

- Remove the always-visible `Variation set` draft grid and `Queue variation set` button from `SourceClipInspector`.
- Keep single generated-source actions: `Queue variation`, `Queue and replace selected clip`, `Queue upscale`, prompt editing, restore-original prompt, generated details, references, source reveal, and generated-output replacement.
- Preserve the existing Temporal-backed variation-set workflow through the Codex composer and selected-source AgentPanel path.
- Keep `EditorWorkspace.queueGeneratedClipVariationSet` and the generated asset/job data contract unchanged.
- Do not remove tests proving selected-source variation sets create one Temporal-backed batch with multiple generated assets.

## Testing

- Update `SourceClipInspector` coverage to assert `Generated AI edit` does not render `Variation set`, `Variation prompt Storm Clouds`, or `Queue variation set`, even when the caller can queue variation sets elsewhere.
- Remove source-inspector-only queue variation set interaction coverage.
- Add or keep workspace/agent coverage proving the Codex composer still routes explicit variation-set prompts through `queueGeneratedClipVariationSet`.
- Run focused source inspector, editor workspace, and agent panel tests; then typecheck, full tests, and browser QA.

## Self-Review

- No project schema, Temporal, fal.ai provider, or generated asset persistence changes.
- The bulk variation workflow is preserved, but it moves out of persistent source-inspector chrome.
- Scope is limited to visible UI and prop wiring for `SourceClipInspector`.
