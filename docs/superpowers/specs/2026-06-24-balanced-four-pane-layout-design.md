# Balanced Four-Pane Layout Design

## Context

The editor now matches Palmier's broad pane model: Codex chat, source library, preview/timeline, and inspector. At the current `xl` desktop breakpoint, the fixed side rail widths leave the preview and timeline center too narrow around 1280px wide. That weakens the manual editing experience because the viewer, transport, toolbar, and timeline become cramped as soon as all four panes appear.

Palmier screenshots keep chat and media visible while still giving the preview and timeline the dominant working area.

## Goal

Keep the four-pane desktop layout, but rebalance column sizing so the center editor has a meaningful minimum width when the four panes are visible.

## Behavior

- The `Editor panes` grid still stacks to one column below the desktop breakpoint.
- At `xl`, the grid uses proportional columns with compact side rail minimums and a `minmax(520px, 1.8fr)` preview/timeline center.
- Codex and source library rails use compact `minmax(220px, 0.7fr)` sizing.
- The inspector uses `minmax(260px, 0.8fr)` sizing.
- No pane order, behavior, or component props change.

## Non-Goals

- No new resizable split panes.
- No collapsing/hiding rail controls.
- No changes inside `AgentPanel`, `MediaBin`, `PreviewPanel`, `TimelineEditor`, or inspectors.
- No schema, workflow, or Temporal changes.

## Testing

- Update the responsive workspace test to assert the balanced four-pane grid class.
- Keep the Codex rail, source library, center editor, and inspector rail accessible in the same order.
- Run full frontend verification and visual smoke checks at desktop and narrow widths.
