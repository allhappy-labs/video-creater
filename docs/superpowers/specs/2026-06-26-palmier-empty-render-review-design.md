# Palmier Empty Render Review Design

## Intent

Palmier keeps the center editor focused on the preview, transport, and timeline until an actual render artifact exists. Video Creater currently shows a dashed `No render report yet.` box between the transport and timeline on every fresh project. That placeholder adds vertical chrome without enabling an action. Hide the empty center render review while preserving real render review reports and the timeline inspector's render summary.

## Requirements

- Before any export render exists, the center preview column must not render the `Render review` region.
- Completed render reports must still render the `Render review` region with job, status, output, quality, graphics, and artifact details.
- The right-rail project timeline inspector must keep its own `No render report` summary when no report exists.
- Preview transport, source viewer, and timeline editor behavior must not change.
- Browser QA must confirm the preview transport sits directly above the timeline area on a fresh project.

## Verification

- Update `EditorWorkspace` tests so the fresh-project center column has no `Render review` region.
- Keep final WebM and draft review report tests passing.
- Browser-smoke the editor and confirm the empty dashed render-review placeholder is gone.
