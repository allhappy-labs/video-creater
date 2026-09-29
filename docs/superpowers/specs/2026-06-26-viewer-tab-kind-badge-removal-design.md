# Viewer Tab Kind Badge Removal Design

## Context

Palmier's viewer tab strip uses concise tab names such as `Timeline` and source filenames. Video
Creater still prepends compact source-kind badges (`AI`, `Video`, `Audio`, etc.) inside source tabs.
Those badges add metadata noise to the tab strip now that source details and transport metadata are
available elsewhere.

This supersedes the visible tab badge behavior from earlier preview-header work. Source kind remains
available in source viewer detail panels and media/library contexts.

## Requirements

- Source viewer tabs render the source label only.
- Source viewer tabs must not render kind badges such as `AI`, `Video`, `Audio`, `Image`, or
  `Generated`.
- Viewer tab navigation, close buttons, active tab styling, tab accessible names, and source detail
  metadata remain unchanged.
- Timeline tabs remain unchanged.

## Validation

- Preview panel component tests assert generated and video source tabs contain only source labels.
- Existing viewer tab navigation and close-tab tests continue to pass.
- Browser QA confirms the viewer strip no longer shows source-kind badges.
