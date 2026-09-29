# Generated Details Single Recipe Readout

## Context

The generated Source Inspector now renders details and AI edit controls inline. The details block still repeats generation recipe metadata twice: a compact `Generated recipe summary` card shows model, aspect, resolution, duration, and status, then a separate `Generated` card repeats model, aspect, resolution, and duration lower in the same inspector.

## Goal

Keep one authoritative generated recipe readout at the top of `Generated details`. The summary should include enough model identity to replace the lower generated settings card, while file metadata, references, workflow status, prompt, and edit controls remain available.

## Requirements

- `Generated recipe summary` remains the first generated-details readout.
- The summary model value includes provider and model id, for example `seedance/seedance-2-fast`.
- The lower duplicate `Generated` settings card is removed from generated details.
- File metadata, references, workflow status, prompt copy, rerun, replacement, variation, variation-set, and upscale actions remain unchanged.
- No project schema, Temporal payload, or fal.ai provider behavior changes.

## Verification

- Source Inspector tests assert the summary carries provider/model identity.
- Source Inspector tests assert generated details no longer render a separate `Generated` settings heading.
- Existing generated Source Inspector behavior tests pass.
- Browser QA confirms the right rail has one generated recipe readout rather than two.
