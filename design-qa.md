# Palmier Editor Design QA

> **Historical (2026-09-15).** This audit covers the pre-redesign editor and is kept only as a
> record. The editor chrome no longer targets Palmier layout parity; the current editor follows the
> [editor UI/UX redesign spec](docs/superpowers/specs/2026-09-13-editor-ui-ux-redesign-design.md).
> Its acceptance gate is the browser fixture flows run by `pnpm test:browser`, with captures in
> `output/editor-acceptance/`. The workflows, geometry, and captures below describe the retired
> editor.

Final result: passed (historical)

## Scope and evidence

- Product under review: Video Creater at `http://127.0.0.1:1420/`.
- Visual source of truth: the ten supplied Palmier screenshots plus the local `reference/` checkout at `cdd63ff`.
- Reference captures: `output/parity-audit-2026-07-13/reference/`.
- Implementation captures: `output/parity-audit-2026-07-13/implementation/`.
- Side-by-side board: `output/parity-audit-2026-07-13/comparison/index.html` with nine embedded pairs.
- Browser coverage: 38 full states and four normalized comparison regions at 1440x960, 1280x720, 1024x768, and 390x844.
- Final regression: 105 frontend suites and 1,366 tests passed, together with TypeScript lint, the production build, the fresh 38-state visual run, and the regenerated nine-pair board.

## Primary workflows verified

- Open the bundled sample project from the project home.
- Navigate Media, Captions, and Audio from the compact source rail.
- Open the attached image/video/audio generator, edit references and settings, and reach the queue action.
- Open the media overflow and reach New Folder, Create Matte, Organize with Agent, and retained silence actions without clipping.
- Configure caption source, mode, language, word limits, profanity, placement, and Agent Mode actions.
- Use speech marking, speaker identification, and silence removal.
- Switch contextual Details and AI Edit inspector tabs.
- Open and close the Codex starter rail without changing the default editor geometry.
- Open export destinations and video settings.
- Exercise timeline focus, hover, resize, drag, mixed track heights, and narrow selected-action states.
- Open profile and Settings at desktop and narrow widths.

## Fidelity review

- Typography: compact hierarchy, weights, and muted metadata follow the Palmier references. Product and sample names intentionally differ.
- Spacing and geometry: 28px panel headers, 38px timeline toolbar, 4-6px upper-deck gaps, compact source rail, viewer-first upper deck, and deep timeline are asserted by the visual harness.
- Color and surfaces: the dark neutral shell, subtle borders, opaque popovers, white primary actions, and muted secondary actions are consistent. Video Creater retains its cyan focus/accent treatment.
- Imagery: Palmier's private sample imagery is not copied. Video Creater uses its bundled public-domain Edison sample and real generated thumbnails.
- Copy and content: Palmier-compatible labels and hierarchy are used where workflows match; truthful provider, runtime, recovery, and project-path copy remains where Video Creater has additional capabilities.
- Icons: existing Lucide application icons are used consistently; no improvised image or glyph assets were introduced.
- Responsive behavior: desktop, laptop, compact desktop, and 390x844 states remain reachable with no root horizontal overflow in the harness.
- Accessibility: tabs, dialogs, menus, menuitems, toolbars, and source regions retain accessible names; the media overflow moves focus to its first enabled action, supports Arrow/Home/End navigation, closes on action, outside click, or Escape, and returns focus on Escape.

## Comparison history

1. The generation drawer was attached to the source panel and given deterministic Palmier-style geometry.
2. Audio navigation was corrected so Speech stays focused while Music and Effects open generation controls.
3. Codex rail state was reset between captures so non-agent comparisons use the default editor layout.
4. Export, generation, captions, and media overflow were normalized into focused same-state comparison regions.
5. The media overflow was found clipped by the source panel, moved into a viewport-level portal, and recaptured with the full menu bounds.
6. The browser-only Tauri `transformCallback` absence was classified as an unavailable bridge; a fresh app load then produced zero console errors.

## Accepted differences

- The Edison sample, local project-path/relink flow, native runtime diagnostics, render-quality controls, provider-specific generation details, and advanced timeline/Codex tools are intentional Video Creater-only capabilities and remain documented in `docs/parity.md`.
- The caption comparison shows a different scroll position because Video Creater preserves the Agent Mode menu in the shorter source-panel viewport; all source and mode controls remain present and covered.
- The result is design-parity at the workflow, hierarchy, density, and responsive-layout level. It is not claimed to be a pixel-identical copy of Palmier's private sample content.

No actionable P0, P1, or P2 design defects remain in the supplied screen set.
