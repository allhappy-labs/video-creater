# Codex Generated Timeline Clip Actions Design

## Context

Palmier keeps AI-generated media editable from the timeline: selecting a generated clip exposes
its prompt, model, references, and rerun or tweak controls without forcing the editor to switch
surfaces. Video Creater already exposes generated-source details in the Source Inspector and
selected-media Codex context, and the Codex selected timeline clip block now supports mention,
split, trim, and reorder operations. The missing gap is that a generated clip selected on the
timeline still looks like a plain source clip inside Codex.

## Goal

When a generated timeline clip is selected, the Codex rail should show compact AI provenance and
generation actions inside the existing `Selected timeline clip` block. The editor should be able
to rerun the original prompt or tweak the selected generated clip using the current Codex prompt
without leaving the chat rail.

## Behavior

- Extend selected timeline clip context with optional generated metadata:
  - generated asset id;
  - model label;
  - prompt;
  - first frame, last frame, and reference media ids.
- Show an `AI clip` subsection only when the selected timeline clip resolves to a generated asset.
- Render model and reference counts as compact chips, and show the original prompt in a clamped,
  copyable provenance block.
- Show `Rerun same prompt` when a generated asset id and queue callback are available. It queues a
  variation using the original generated prompt.
- Show `Queue variation` when a generated asset id and queue callback are available. It queues a
  variation using the trimmed Codex prompt and is disabled while the prompt is blank.
- Keep existing selected clip mention, split, trim, and reorder controls unchanged.

## Non-Goals

- No new project schema, Rust action, Temporal workflow contract, fal.ai provider behavior, or MCP
  tool contract.
- No automatic replacement of the selected timeline clip after queueing.
- No multi-output chooser; generated output replacement remains in Source Inspector and selected
  media context.
- No image thumbnails in the Codex block for this slice. The rail stays dense and text-first.

## Data Flow

1. `EditorWorkspace` resolves the selected source clip to a generated asset using the same media id
   and timeline properties used by Source Inspector.
2. `EditorWorkspace` passes optional generated fields through `AgentSelectedTimelineClipContext`.
3. `AgentPanel` renders the AI subsection and calls `onQueueSelectedVariation(assetId, prompt)`.
4. Existing `queueGeneratedClipVariation` keeps preserving model, settings, references, parent
   asset id, retry asset id, placement intent, and Temporal start request behavior.

## Validation

- `AgentPanel` renders generated timeline clip model, prompt, reference count, rerun, and tweak
  controls.
- `AgentPanel` rerun calls the variation callback with the selected generated asset id and original
  prompt.
- `AgentPanel` tweak calls the variation callback with the selected generated asset id and trimmed
  Codex prompt.
- `EditorWorkspace` passes generated metadata for a selected generated timeline clip and queues a
  variation from the Codex selected-clip block.
- Existing selected timeline clip mention, trim, split, and reorder tests continue passing.
