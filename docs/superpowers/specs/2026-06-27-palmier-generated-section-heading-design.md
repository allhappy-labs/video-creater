# Palmier Generated Section Heading Design

## Context

Palmier's source inspector separates generated media facts into visible rail sections: `Details`, `References`, `Generated`, and `Prompt`. Video Creater flattened generated file metadata into direct rows, but the generated model/output facts now sit under an ARIA-only `Generated details` wrapper with no visible section heading. That makes the right rail less scannable than the Palmier reference and hides the distinction between file facts, references, generation facts, and prompt.

Palmier reference: https://www.palmier.io/docs

## Goal

Add a visible `Generated` heading above generated file metadata rows while keeping the direct Palmier-style row layout.

## Requirements

- The Source Inspector generated source readout shows `Generated` as a compact section heading before model/status/aspect/path/type/duration/resolution/frame-rate rows.
- The heading is visible text inside the generated details wrapper.
- The file metadata remains one flat `Generated file details` group.
- Do not reintroduce nested `Generation` or `Format` groups.
- Do not add tabs, toggles, switches, cards, or extra action rows.
- Existing `References`, `Prompt`, and `Generated AI edit` controls remain unchanged.

## Tests

- `SourceClipInspector` proves generated details include the visible `Generated` section heading.
- Existing flat metadata tests prove `Generated file details` still has no nested `Generation` or `Format` groups.
- `EditorWorkspace` proves generated source details opened from the AI media overview include the `Generated` section before prompt/reference details.
