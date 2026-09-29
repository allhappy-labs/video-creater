# Source Inspector Rail Shell Flattening

## Context

Palmier's right rail renders inspector content directly under the rail tabs. Video Creater has already
removed the duplicated `Source Inspector` title, but `SourceClipInspector` still wraps source-mode
content in its own rounded card. Inside the already-framed inspector rail, that extra shell makes the
source view feel heavier than the reference and consumes space that should belong to source metadata,
references, and edit controls.

## Goal

Flatten the source-mode `SourceClipInspector` shell so the right rail owns the outer framing while the
inspector body keeps its accessible region name and existing content structure.

## Requirements

- Source-mode `SourceClipInspector` remains an accessible `Source Inspector` region.
- Source-mode `SourceClipInspector` no longer renders the outer rounded card shell classes.
- The timeline variant keeps its standalone card treatment and visible `Timeline` heading because it
  can appear outside the right rail.
- Generated, imported, and selected timeline source content remains unchanged inside the inspector.
- Right-rail source/timeline switching behavior does not change.

## Verification

- Source inspector component tests assert source mode is not wrapped in the old rounded card shell.
- Source inspector component tests assert the timeline variant still uses the standalone shell and
  visible heading.
- Focused source inspector tests pass.
- Browser QA confirms the right rail starts with inspector content under the `Source` tab without a
  nested card outline.
