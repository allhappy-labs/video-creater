# Frontend interaction fixes evidence

## Scope and behavior

- Viewer transport ownership is explicit. `PreviewPanel.interactionEnabled` gates the source/timeline preview document listener, while `TimelineEditor.keyboardShortcutsEnabled` gates all hidden-workspace shortcuts and `transportShortcutsEnabled` assigns timeline transport only when the timeline viewer is active. Both listeners honor `defaultPrevented`.
- Viewer tabs now use roving `tabIndex`, linked tab/tabpanel IDs, Left/Right/Home/End navigation, and deterministic focus restoration after closing the active source tab. Handled tab keys stop before document transport listeners.
- Settings internal navigation records the external request generation that created it. A later external request takes precedence synchronously even when its numeric request ID equals the internal request ID.
- Canonical preview suppression now receives per-frame coverage. Ready-without-sequence, sequence gaps, and prepared-media mismatches show a recoverable `Prepared frame missing` state and never reveal the direct approximation. Flattened composite frames cover original rich layers removed from the prepared timeline without masking a surviving prepared layer whose own frame is missing.
- Timeline media clock correction is separate from play/pause transitions. One play request remains pending per item/source generation, source replacement can start while an obsolete request is pending, rejection waits for a metadata or transport transition before retrying, and unmount marks transport stopped and pauses an active element.
- Timeline grid lines and ruler edit-point markers are hidden from the accessibility tree. The timeline canvas retains its stable accessible name and provides duration/zoom through `aria-description`.

## Regression evidence

Red run before implementation reproduced: three `play()` calls during repeated clock samples; no canonical-ready coverage alert; stale Settings internal navigation; source handling of a prevented arrow; missing viewer tab semantics/focus; and exposed decorative markers.

Green focused run:

```text
pnpm vitest run \
  src/components/workspace/timeline-editor.test.tsx \
  src/components/workspace/preview-panel.test.tsx \
  src/components/workspace/timeline-preview-compositor.test.tsx \
  src/components/settings/settings.integration.test.tsx

Test Files  4 passed (4)
Tests       348 passed (348)
```

`git diff --check` passed. After Task 1 integration, `pnpm lint` passed both browser and Node TypeScript projects.

## Shared wiring contract

The Task 1 owner wires the retained workspace with:

```tsx
<PreviewPanel interactionEnabled={isActive} />
<TimelineEditor
  keyboardShortcutsEnabled={isActive}
  transportShortcutsEnabled={isActive && viewerMode === "timeline"}
/>
```

No canonical project mutation moved into browser code. Canonical coverage is presentation/recovery state derived from the Rust-prepared timeline and frame sequences. Runtime performance on packaged macOS and physical target hardware remains unverified from Ubuntu.

## Inspector drafts and resize

- A shared draft lifecycle compares only editable values. Unrelated project snapshots retain dirty caption, text-overlay, trim, source-property, and visual-effect drafts; selection identity changes reset them.
- A real external edit preserves the local draft, shows plain-language recovery copy, disables affected apply actions, and offers `Reload latest values`. The reload adopts the newest project values rather than submitting a stale multi-field update.
- Visual-effect draft creation uses the same focused lifecycle, including catalog snapshots and selected-clip changes.
- Pointer resize samples are coalesced through `requestAnimationFrame`. `InspectorDock.onResizePreview` drives the live workspace grid column, while `onResize` persists only the final pointer-up width. Pointer cancel and lost capture restore the starting width and clear the preview. Keyboard resize remains immediate and persisted.

Task 4 focused evidence:

```text
pnpm vitest run \
  src/components/workspace/use-inspector-draft-lifecycle.test.tsx \
  src/components/workspace/caption-inspector.test.tsx \
  src/components/workspace/text-overlay-inspector.test.tsx \
  src/components/workspace/source-clip-inspector.test.tsx \
  src/components/workspace/inspector-dock.test.tsx

Test Files  5 passed (5)
Tests       130 passed (130)

pnpm vitest run src/components/workspace/editor-workspace.test.tsx \
  -t "previews inspector pointer"

Test Files  1 passed (1)
Tests       1 passed (334 skipped)
```

One combined 813-test component run passed 811 tests and hit timeouts in two pre-existing long EditorWorkspace interactions. Both timed-out cases passed immediately in isolation. The Task 4 workspace resize regression and all five inspector suites passed in that combined run.

Final architecture review found two interaction edge cases. Source Clip now reconciles drafts per Apply action, so a returned visual-fade update cannot conflict with duplicated hidden audio-fade fields and applying one group preserves a dirty independent group. A true external change to that dirty group still requires reload. Viewer close focus now follows the selected tab ID returned by the workspace owner, including closing the middle source in a three-source strip. The inspector resize test also restores its animation-frame spies so it cannot affect later interaction tests.

```text
pnpm vitest run src/components/workspace/source-clip-inspector.test.tsx \
  src/components/workspace/preview-panel.test.tsx

Test Files  2 passed (2)
Tests       154 passed (154)

pnpm vitest run src/components/workspace/editor-workspace.test.tsx \
  -t "previews inspector pointer|duplicates an Option-drag|implements standard keyboard navigation"

Test Files  1 passed (1)
Tests       3 passed (332 skipped)
```
