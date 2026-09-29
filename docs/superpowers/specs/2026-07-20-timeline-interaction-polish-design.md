# Timeline Interaction Polish Design

## Status

Autonomously approved for implementation on 2026-07-20. The user explicitly delegated design decisions and requested that specification and implementation proceed without additional input.

This specification is a focused follow-up to `2026-07-18-timeline-ux-redesign-design.md`. The existing viewport model, canonical edit evaluator, collision policy, live preview, track geometry, overview navigator, keyboard behavior, and project mutation contracts remain authoritative.

## Goal

Make the current timeline feel direct and legible in ordinary editing: clip information must not fight filmstrips or waveforms, resize targets must be discoverable without pixel hunting, feedback must stay inside the interaction surface, vertical scrolling must retain track context, browser coverage must prove committed edits, and long-project interaction checks must enforce an actual responsiveness budget.

## Audit Evidence

Fresh sample-project browser captures on 2026-07-20 confirmed these residual problems:

- audio title, timing, fade copy, automation, and waveform content occupy the same visual layer;
- video titles and status badges compete with bright filmstrip content;
- resize handles are rendered only for selected clips, so an unselected clip appears non-resizable;
- the resize readout can occupy the ruler-adjacent area and compete with time labels;
- constrained-height captures can show only the lower tracks without a persistent indication of which track range is visible;
- drag, resize, selection-drag, and collision fixtures prove transient states but release through cancellation, not through a committed edit followed by undo;
- the long-project fixture records markers but does not measure or enforce interaction latency.

The focused timeline baseline is healthy: 212 timeline editor, interaction-frame, and viewport tests pass before this work.

## Approaches Considered

### 1. Incremental DOM timeline hardening — selected

Keep the existing accessible DOM timeline and improve its presentation and browser contracts. This has the lowest semantic risk because pointer, keyboard, collision, undo, and Rust validation already share the intended architecture. It also lets the work land as independently testable changes.

### 2. Canvas-rendered clip bodies

Move filmstrips, waveforms, labels, and interaction chrome into a canvas. This could reduce DOM work, but it would require recreating focus, accessible names, pointer hit testing, resize controls, and status inspection. It is disproportionate to the confirmed residual problems.

### 3. Separate DOM interaction overlay over a canvas timeline

Retain accessible controls in the DOM while drawing the full timeline beneath them. This is a plausible future direction for extremely large projects, but it introduces two geometry systems and a new synchronization boundary. The existing viewport virtualization already bounds the current DOM, so this pass does not justify the architectural cost.

## Scope

This pass changes:

- adaptive clip-body information layers in `TimelineItemShell`;
- audio fade and waveform presentation inside timeline clips;
- pointer and keyboard availability of resize controls;
- resize feedback placement and copy;
- track-viewport orientation at constrained heights;
- headed browser scenarios for committed move, committed resize, undo, and long-project latency;
- focused unit, component, source-policy, visual, and build verification.

It does not change:

- timeline or project schemas;
- the canonical move/resize/trim evaluator;
- snapping, minimum duration, collision, group movement, ripple, or source-range semantics;
- inspector, preview, render, export, or Codex workflows;
- touch-first or mobile behavior;
- the overview navigator architecture;
- native Rust validation rules already covered by the 2026-07-18 redesign.

## Clip Readability

### Information hierarchy

Clip bodies continue to reveal content by rendered width:

- below 48 pixels: accent only, with the full accessible name and tooltip;
- 48–119 pixels: one truncated title line;
- 120–219 pixels: title and compact timeline timing;
- 220 pixels and wider: title, timing, and concise status/provenance.

For rich video and audio clips, the title/timing/status cluster sits on a localized dark scrim above the filmstrip or waveform. The scrim must not become a full-width opaque slab. It follows the content width, uses a restrained translucent background and soft edge, and preserves the underlying media as the dominant texture.

The readable text cluster has explicit horizontal inset from resize hit regions. Long labels and reasons truncate within that cluster rather than spilling into handles, adjacent clips, or the timeline ruler.

### Audio-specific treatment

Waveform peaks remain visible as the primary audio texture, but they sit below the readable text cluster and use reduced opacity behind it.

Fade ramps remain visual and accessible. The ramp does not paint persistent `fade out 0.75s` copy over the waveform. Fade duration remains available through the ramp accessible name, clip tooltip metadata, inspector, and selected fade controls.

Automation and fade handles remain above the waveform. Their hit regions must not cover the clip title cluster or either resize edge.

## Resize Discoverability And Acquisition

Editable clips at least 48 pixels wide expose left and right resize hit regions before selection. The visible rule appears on clip hover, body focus, handle focus, selection, or an active resize. A user can therefore move directly to an edge and begin resizing without a prior selection click.

Pointer-down on an unselected resize edge synchronously selects that clip before it captures the pointer. The inspector, accessible resize values, live preview, and eventual patch therefore share one authoritative item.

Each edge hit region is 20 CSS pixels wide and spans the clip height. The visible rule remains 3 pixels wide, so precision does not make the timeline visually heavy.

Unselected resize handles use `tabIndex=-1` to avoid adding two keyboard stops for every clip. Selecting or keyboard-focusing the clip makes its handles keyboard reachable. Existing left/right accessible names, values, descriptions, arrow-key resize behavior, and focus rings remain.

Clips narrower than 48 pixels keep the existing selected compact dock. They require selection before the dock appears because two persistent edge hit regions would consume the entire clip. The accessible name and tooltip continue to identify the clip before selection.

Locked tracks never expose active resize hit regions. Disabled tracks retain current editing semantics and visual treatment.

## Interaction Feedback

Resize feedback stays inside the active clip. It never renders above the clip into the ruler lane. The bubble uses compact copy:

`Right 00:00:04.500 · 4.500s`

or the corresponding left-edge value. The accessible label retains the explicit edge time and duration wording.

The bubble clamps to the clip body, uses a maximum width, truncates only the visible copy when unavoidable, and remains pointer-transparent. While it is present, ordinary clip text may yield beneath it; interaction truth has priority over static metadata.

Move and invalid-target messages retain their existing target-local lanes and live-region announcements.

## Vertical Track Orientation

The track viewport uses a stable scrollbar gutter where supported and publishes a compact orientation readout in the fixed ruler/header column. The readout shows:

`Tracks 3–5 of 5`

It updates from the current `scrollTop`, viewport height, and canonical track geometry. If every track is visible, it reads `5 tracks` rather than a redundant range.

The readout is programmatically named `Timeline visible track range`. It does not become an extra button, does not alter scroll position, and does not rerender during pointer movement. Track headers and canvas rows continue to share the same vertical scroll container, so the range is computed from one authoritative viewport.

## Interaction Persistence Coverage

The headed browser suite adds two release-based scenarios:

### Committed move and undo

1. Record `Opening clip` canonical position.
2. Drag it to a valid non-overlapping position.
3. Release the pointer.
4. Wait for canonical geometry to match the preview.
5. Verify exactly one undoable project action is reflected in the UI.
6. Invoke Undo.
7. Verify the original geometry is restored.

### Committed resize and undo

1. Select `Opening clip` in a fixture with right-side room.
2. Record its canonical width and right-edge accessible value.
3. Drag the right edge to a valid duration.
4. Release the pointer.
5. Wait for canonical geometry and accessible value to match the preview.
6. Invoke Undo.
7. Verify the original width and value are restored.

These scenarios validate the complete React workspace/project state path used by the browser fixture. Native persistence and Rust overlap validation remain covered by the existing project-action and Rust suites; the browser fixture does not claim save/reopen native acceptance.

## Long-Project Responsiveness Contract

The deterministic 30-minute browser fixture measures representative move and resize gestures with the browser's monotonic clock. For each gesture it records:

- elapsed time from pointer movement until two animation frames have completed;
- the number of rendered grid elements;
- the number of mounted timeline item shells;
- the presence of the expected live geometry.

Acceptance limits:

- each measured gesture completes in at most 50 ms in the local headed fixture;
- combined ruler/grid element count remains at most 400;
- mounted timeline items remain bounded to the visible/overscanned window plus persistent selected or active items;
- move and resize still expose exactly one trace marker each.

The 50 ms check is intentionally local and deterministic. It is a regression guard, not a claim about native rendering, GPU scheduling, or every host.

## Component Boundaries

`TimelineItemShell` owns adaptive content hierarchy and resize-control presentation. It does not decide edit semantics.

`TimelineEditor` decides which items are editable, provides geometry, computes visible track range, coordinates transient interaction state, and delegates the existing callbacks.

The browser QA script owns full-flow release/undo and deterministic latency assertions. It must not add production-only hooks when the same state is observable from geometry, accessible values, toolbar state, and existing fixture markers.

## Error Handling

- Rejected moves and resizes continue to emit no patch.
- A browser scenario fails if canonical geometry does not settle after release.
- A browser scenario fails if Undo does not restore the recorded geometry.
- A performance scenario reports measured move/resize duration and node counts in its thrown error.
- Resize pointer cancellation and lost capture retain existing rollback behavior.
- Orientation readout clamps its first and last visible indices to the canonical track count, including an empty timeline.

## Accessibility

- Clip titles retain full accessible names and tooltips at every visual density.
- Direct pointer resize before selection does not add keyboard tab stops.
- Selected resize handles retain edge, value, description, arrow-key behavior, and visible focus.
- Fade metadata remains available without persistent overlapping text.
- Resize feedback remains announced without taking focus.
- The visible track range is text, not color or scrollbar position alone.
- Reduced-motion behavior remains unchanged.

## Verification

TDD cycles must first prove:

- rich media clip text uses the readable scrim and respects the resize inset;
- audio fade ramp no longer renders persistent copy over the waveform;
- an unselected wide editable clip exposes pointer resize hit regions with non-tabbable handles;
- selecting the same clip makes both handles keyboard reachable;
- locked clips do not expose resize controls;
- resize feedback is in-clip and uses compact visible copy while retaining its explicit accessible label;
- visible track range calculation handles all-visible, middle, final, and empty cases;
- the track viewport exposes its orientation readout and stable-gutter treatment;
- browser move and resize release to canonical geometry and Undo restores the original;
- long-project browser checks enforce the 50 ms and node-count budgets.

After focused red/green cycles, run:

- the complete `TimelineEditor` suite;
- timeline viewport and interaction-frame tests;
- modern editor visual fixture and browser-scenario source tests;
- headed drag, resize, collision, long-project, 1280×720, and 1024×720 captures;
- TypeScript lint;
- production build;
- the relevant Rust timeline action suite if no Rust sources changed, to confirm cross-layer semantics remain stable.

## Acceptance Criteria

This pass is complete when:

- media texture and readable clip metadata no longer occupy the same unprotected visual layer;
- audio waveform, fade, title, and timing are visually separable;
- a wide unlocked clip can be resized directly from either edge before selection;
- selected and keyboard resize behavior remains intact;
- resize feedback never competes with ruler labels;
- constrained-height scrolling communicates which track range is visible;
- headed browser flows prove committed move, committed resize, and Undo;
- the deterministic long-project check enforces an interaction and DOM budget;
- focused tests, complete timeline tests, lint, build, browser visual checks, and cross-layer Rust tests pass;
- unrelated Settings and native-runtime work remains untouched.
