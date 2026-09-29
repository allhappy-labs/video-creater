# Timeline and Preview Interaction Design

Date: 2026-07-22  
Status: Approved interaction direction; ready for implementation planning

## Outcome

Make the timeline preview feel like one coherent editing surface. A selected layer has one explicit canvas tool, playback and scrubbing drive every visual layer from one timeline clock, the playhead remains usable at every zoom and scroll position, and the preview always shows a meaningful frame—including at the exact end of the timeline.

This is a coordination and interaction-quality project, not a panel-resizing project.

## Evidence Reviewed

The audit used the running app and recorded the following local evidence in `output/playwright/timeline-preview-audit/`:

| Evidence | State | Finding |
| --- | --- | --- |
| `01-current-state.png` | Selected media | Transform, crop, and rotate affordances compete on the same canvas. |
| `02-playhead-5s.png` | Playhead outside selected clip | Selection chrome remains even when the selected layer is not active at the playhead. |
| `03-template-panel.png` | Template browser | Confirms the Holographic Logo Cutout used for the motion audit. |
| `05-after-undo.png` | Selection cleared | Canvas toolbar can remain visually stale after the inspector reports no selection. |
| `06-active-playback.png` | Exact timeline end | Preview falls through to “No timeline media at playhead.” |
| `08-controlled-playback.png` | Playing selected media | Selection controls remain present while content is playing. |
| `09-holographic-start.png` | Holographic at 2.000 s | Initial template frame. |
| `10-holographic-playing.png` | Holographic at 2.815 s | Template remains visually static while captions and timeline time advance. |

The crop keyboard flow was not accepted as verified evidence because another application stole focus during capture. Crop behavior remains in scope based on the directly observed pointer UI and current implementation.

## Issue Inventory

### P0 — Motion templates do not use timeline time

`MotionTemplatePreview` accepts only a timeline item. `TimelinePreviewCompositor` renders templates without the current timeline time or playback state. The workspace playback loop advances the playhead correctly, but template visuals have no time input and therefore cannot reconstruct a frame during play or scrub.

Impact: built-in motion templates appear broken and exported-motion expectations cannot be previewed accurately.

### P1 — Canvas tools are simultaneous instead of modal

The current `CanvasSelectionOverlay` renders move, four resize controls, rotate, and four crop controls together. Cyan transform bounds and amber crop bounds overlap, producing nine small tabbable manipulation controls plus the full-body move target.

Impact: unclear editing intent, crowded focus order, small pointer targets, and high risk of invoking the wrong operation.

### P1 — Selection chrome is disconnected from playhead state

Canvas controls can remain visible after the selected item is no longer active at the current playhead or after selection has been cleared elsewhere.

Impact: the preview implies an inactive or nonexistent layer is directly editable.

### P1 — Exact timeline end produces an empty preview

Timeline activity uses an exclusive end boundary: `playhead < start + duration`. At the project duration, no final media item is active, so the preview displays an empty-state message instead of the last composited frame.

Impact: jumping to end looks like media disappeared and undermines confidence in the edit.

### P1 — Playhead identity is easy to lose

The playhead head sits inside the scrollable clip canvas and the visible time indication can be clipped or truncated. The available seek surfaces are visually thin.

Impact: precise navigation is difficult, especially at timeline edges, high zoom, and constrained widths.

### P1 — Core targets are undersized

Observed canvas controls are approximately 29×29 px, preview transport controls approximately 28×29 px, and the visual scrubber approximately 5 px high.

Impact: avoidable pointer error and weak touch/assistive accessibility.

### P2 — Playback does not simplify the canvas

Direct-manipulation chrome remains visually dominant while the user is evaluating motion.

Impact: the edit cannot be reviewed cleanly and selection outlines obscure motion quality.

### P2 — Inactive-selection feedback is missing

When the timeline selection is outside the playhead, there is no concise explanation of why canvas manipulation is unavailable.

Impact: users may repeatedly reselect or assume editing is broken.

## Approved Interaction Model

### 1. Explicit canvas modes

The preview toolbar has mutually exclusive modes:

- **Transform**: move body plus four corner resize handles.
- **Crop**: four crop handles, dimmed cropped-away region, aspect controls, and reset.
- **Rotate**: rotation affordance and numeric angle feedback.
- **Fit**: immediate fit/fill/original-size action or compact menu; it does not add canvas handles.
- **More**: secondary item actions only.

Only the active mode contributes interactive canvas controls or tab stops. The selected layer keeps one outline whose visual treatment matches the active tool.

The initial mode is Transform. A selected layer may remember its last used mode until playback starts, the selection changes, or the project closes. Playback temporarily hides edit chrome; pausing restores the remembered mode only if that selected layer is active at the current frame.

### 2. Canvas target and keyboard contract

- Interactive hit regions are at least 44×44 px, even when the visible handle is smaller.
- Arrow keys adjust the focused control by 1%.
- Shift+Arrow adjusts by 5%.
- Rotation retains an angle-specific increment and Shift snapping behavior.
- Escape cancels the active pointer gesture and restores its starting values.
- Pointer-up and each keyboard step commit through the existing project-action path so changes remain undoable.
- Focus labels name the operation, layer, and edge or corner.
- Hidden modes contribute no focusable descendants.

### 3. Selection and playhead relationship

Timeline selection and playhead position remain independent. The preview applies this display rule:

| Selection state | Playhead state | Preview behavior |
| --- | --- | --- |
| Selected layer is active | Paused | Show toolbar and active-mode controls. |
| Selected layer is active | Playing | Hide toolbar and edit controls; keep timeline selection. |
| Selected layer is inactive | Any | Hide stale controls and show a short clip-range explanation. |
| No layer selected | Any | Show no layer toolbar or controls. |

Inactive-selection message: “{Clip name} is selected at {start}–{end}. Move the playhead into the clip to edit it on canvas.”

The message is transient preview guidance, not an error alert. It must not block transport or timeline input.

### 4. One authoritative preview clock

The timeline playhead is the source of truth for:

- Video and audio media time
- Captions and text layers
- Static overlays
- Motion-template phase
- Transport timecode
- Scrub and step results

Every time-dependent preview layer receives a deterministic time derived from the same playhead. Motion-template time is relative to the template item start and clamped to its duration. A template frame at a given timeline time must be identical whether reached by playback, pointer scrub, keyboard step, or direct seek.

`MotionTemplatePreview` therefore needs at minimum:

- `timeSeconds`: time relative to the template start
- `durationSeconds`
- `isPlaying`
- Optional reduced-motion information if not handled entirely by CSS/media query

Time determines the visual frame; `isPlaying` controls only behavior that cannot be reconstructed from time. Avoid free-running CSS animations whose phase resets on rerender or seek.

### 5. Playback and scrubbing behavior

- While playing, edit chrome hides and a quiet “Playing” state may remain.
- The existing animation-frame loop remains the single clock driver.
- Scrubbing updates all composited layers immediately.
- Pausing holds the exact composited frame without resetting template motion.
- Space toggles play/pause when focus is not inside a text input.
- Left/Right step by the editor frame interval while the timeline or preview transport is focused.
- Jump-to-start and jump-to-end remain explicit transport actions.
- Manual horizontal scroll suspends playhead auto-follow briefly; playback resumes follow only after the playhead leaves the safe region.

### 6. End-of-timeline behavior

The user-facing timeline duration remains exact. Internally, preview evaluation at the exact duration resolves to the last representable preview instant for visual composition. This is a preview-boundary rule, not a change to canonical item durations.

Expected behavior:

- Jump to end displays the last composited frame.
- Playback stops at the exact duration and holds that frame.
- Captions or overlays whose exclusive end equals project duration render their final valid state.
- “No timeline media at playhead” appears only when the timeline genuinely has no visual layer at the resolved preview instant.

Centralize this normalization before frame construction rather than weakening every item interval to an inclusive end, which would make adjacent clips overlap at edit points.

### 7. Playhead and timeline navigation

- Reserve a ruler/overlay lane above clip content for the playhead head and time bubble.
- Keep the complete time bubble inside the viewport by shifting it at left and right edges.
- Draw the playhead line across the visible track canvas.
- Give the ruler a 44 px interaction height, even if its visible ticks remain compact.
- During playback, keep the playhead within a central safe region rather than scrolling every frame.
- Do not auto-follow during an active manual scrub.
- Preserve the current timeline zoom and scroll unless following is necessary.
- Expose the precise playhead time through an accessible name/value and a non-truncated visual label during direct manipulation.

### 8. Transport accessibility

- Each transport control has a minimum 44×44 px target.
- Icons retain explicit accessible labels and visible tooltips.
- Disabled controls remain distinguishable without relying on color alone.
- Timecode has enough reserved width to avoid truncation at the supported maximum project duration.
- The scrubber receives a larger invisible hit region and a clear focus indicator.
- Fit remains separate from temporal transport and exposes its current mode.

## Architecture Boundaries

Primary implementation surfaces:

- `src/components/workspace/editor-workspace.tsx`: authoritative playback state and playhead updates.
- `src/lib/timeline-preview.ts`: preview-time normalization, active-item resolution, and frame building.
- `src/components/workspace/timeline-preview-compositor.tsx`: explicit canvas modes, selection-state gating, target sizing, and template time handoff.
- `src/components/workspace/motion-template-preview.tsx`: deterministic time-based variants.
- `src/components/workspace/preview-panel.tsx`: transport target sizing, timecode layout, scrubber interaction surface, and playback chrome rules.
- `src/components/workspace/timeline-editor.tsx`: ruler lane, playhead bubble, edge clamping, and follow behavior.

Keep project mutations flowing through existing commit callbacks. Do not introduce a parallel canvas state that can diverge from canonical timeline item properties.

## Implementation Slices

Implementation should proceed in small visual slices so each behavior can be exercised in the open app:

1. Normalize preview time at the exact timeline end and hold the final frame.
2. Pass relative time and playback state into motion templates; animate the Holographic variant deterministically.
3. Introduce explicit Transform/Crop/Rotate modes and remove inactive controls from pointer and keyboard interaction.
4. Gate preview chrome by selection activity and playback state; add inactive-selection guidance.
5. Increase transport and scrubber hit regions without growing the preview panel.
6. Add the playhead overlay lane, complete time bubble, edge clamping, and safe-region follow behavior.
7. Perform a cross-state visual/accessibility pass across media, captions, templates, crop, playback, scrub, undo, end frame, zoom, and constrained width.

The full repository test/lint/build suite is intentionally deferred until the user declares the planned UI batch complete. During these slices, use real-app visual verification and only narrow checks essential to the specific change.

## Acceptance Matrix

| Scenario | Acceptance |
| --- | --- |
| Select visual clip | Transform is active by default; only transform controls are interactive. |
| Enter Crop | Transform/rotate controls disappear from canvas and tab order; cropped area is dimmed. |
| Enter Rotate | Only rotation manipulation is active; current angle is visible. |
| Start playback | All edit chrome hides; media, captions, and templates advance together. |
| Pause | Exact current composite remains; applicable edit mode returns. |
| Scrub Holographic template | Multiple timeline positions produce distinct, repeatable visual frames. |
| Seek backward | Template returns to the same earlier visual frame instead of continuing a free-running phase. |
| Selected clip outside playhead | No stale controls; clip-range guidance appears. |
| Clear selection | Toolbar and selection overlay disappear immediately. |
| Jump to exact end | Final composite remains visible at exact duration. |
| Drag playhead near edges | Head and full time label stay visible and usable. |
| Play through scrolled timeline | Auto-follow acts only outside the safe region and does not fight manual scroll. |
| Keyboard canvas edit | Arrow/Shift+Arrow increments are predictable; Escape cancels the active gesture. |
| Keyboard transport | Space and step keys work in editing context and do not hijack text inputs. |
| Constrained width | Transport timecode and essential controls remain accessible without being cut off. |

## Non-goals

- Resizing the overall preview or timeline panels.
- Redesigning the entire editor information architecture.
- Changing canonical timeline duration or clip interval semantics.
- Replacing the existing project action/undo system.
- Building a separate animation engine for templates.
- Running the full validation suite during rapid visual iteration.

## Decisions Recorded

- Chosen direction: explicit canvas modes.
- Canvas contract: approved in the HTML visual companion.
- Timeline/playhead/live-motion contract: approved in the HTML visual companion.
- Design medium: local HTML visual companion; no Figma dependency.
