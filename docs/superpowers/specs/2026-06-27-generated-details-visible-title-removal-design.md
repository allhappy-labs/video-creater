# Generated Details Visible Title Removal

## Context

Palmier-style inspector rails keep metadata grouped with compact section labels instead of repeating
generic panel titles inside each block. Video Creater's generated Source Inspector already exposes an
accessible `Generated details` group, but it also renders a visible `Generated details` label above
the concrete `File`, `References`, and `Prompt` sections. That visible title duplicates the
accessible group name and adds one more line of rail chrome.

## Goal

Remove the visible `Generated details` title from generated Source Inspector content while retaining
the accessible group, generated actions, output choices, and concrete metadata sections.

## Requirements

- The generated details group remains accessible as `Generated details`.
- The visible generic `Generated details` text is not rendered inside that group.
- `Generated recipe summary`, generated quick actions, rerun/replace/insert/output actions, `File`,
  `References`, `Prompt`, and `Generated AI edit` remain unchanged.
- Workspace tests continue to verify generated source content through concrete section labels rather
  than the removed generic title.

## Verification

- Source inspector tests assert the accessible group exists and no longer contains visible
  `Generated details` text.
- Source inspector and workspace tests assert `File`, `References`, `Prompt`, and generated actions
  still render.
- Browser QA confirms the right rail generated inspector flows from actions into concrete sections
  without the redundant title line.
