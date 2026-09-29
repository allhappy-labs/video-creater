# Editor Redesign 09 — Mobile Polish, Acceptance Flows, and Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level. Expand each task into bite-sized TDD steps with exact code before executing it.

**Goal:** Close the redesign.
- Finish the touch and phone interaction details.
- Complete the deterministic fixture transport so all eight spec acceptance flows run in Playwright at 1440×900 and 402×874. Desktop flow 5 must also be completable keyboard-only.
- Enforce the source-size and styling rules with policy tests.
- Remove the temporary knip block and clean up legacy `localStorage` keys.
- Retarget the Linux desktop smoke script.
- Update the product docs, backlog, and the `video-creater-visuals` skill to describe the new editor.

**Architecture:**
- Fixture handlers live in `src/lib/runtime/fixtures/*`, split by domain (plans 06 and 07 added some). They share one in-memory fixture project store, `fixture-project-store.ts`, which applies actions with the real `applyProjectActionsLocally`.
- The acceptance specs are a single ordered Playwright file per viewport, reusing helpers from `e2e/support/editor.ts`.
- Policy tests run inside `test:source-quality`.

**Tech Stack:** Playwright 1.63, Vitest, Node test runner, React 19, Tailwind.

**Spec sections:** Mobile Layout, Acceptance, Engineering Strategy, Risks.
**Depends on:** plans 01–08.

## Global Constraints

- Prefix commands with `rtk`. Use Conventional Commits and stage only the named files.
- Fixture handlers ship only in DEV builds. Bootstrap already gates on `import.meta.env.DEV`. Add a `vite-build-policy` assertion that production bundles contain no `fixtures/` module ids.
- Acceptance flows must not use test-only store hooks. Drive them through the UI only.
- Documentation states evidence boundaries honestly. Browser fixture flows are the acceptance gate for this redesign. Native Tauri checks from plans 06–08 are recorded evidence, not release proof.
- Do not change `verify:frontend` or `verify:release` strings. `scripts/zero-debt-gate.test.ts` pins them. Add new checks inside existing composite steps such as `test:source-quality` and `test:browser`.

---

## File Map

### New files

**Fixture runtime (`src/lib/runtime/fixtures/`)**
- `fixture-project-store.ts`: in-memory project, revision, action application, snapshot save, job progression clock.
- `project-fixtures.ts`: handlers for:
  - `load_split_project_from_folder`, `save_split_project_to_folder`
  - `apply_project_actions_to_split_project_folder`, `apply_project_action_to_split_project_folder`
  - `import_media_to_project` (adds `e2e/fixtures/preview.webm` and a fixture PNG as media)
  - `prepare_project_preview` (returns no frame sequences)
  - `cache_timeline_filmstrip_in_split_project_folder` (returns empty)
  - `list_visual_effect_catalog`, `list_shader_background_templates`
  - `search_project_media` (local matching)
- `speech-fixtures.ts`: handlers for:
  - `build_temporal_transcribe_media_start_request`, `start_temporal_workflow`
  - transcription completion after 2 polls, which adds a deterministic transcript to the fixture project
  - speaker registry read and rename
- `generation-fixtures.ts`: handlers for:
  - `list_generation_model_catalog`
  - `run_generate_media_in_process` (queued → completed after 2 polls with a fixture output)
  - cancel, `complete_mock_generated_asset_in_split_project_folder`
- `index.ts`: registers all domains, keyed by marker flag `acceptanceFixture: true`. This flag implies `conversationFixture` and `exportFixture`.
- A `*.test.ts` for each module.

**Playwright**
- `e2e/support/editor.ts`: `openAcceptanceEditor(page, viewport)`, `collectConsoleErrors(page)`, `clipCount(page)`, `mod(page)` (Meta/Control), `openTab(page, name)` (tab on desktop, bottom tool bar on phone).
- `e2e/editor-acceptance.spec.ts`: flows 1–8, parameterized by viewport.
- `e2e/editor-keyboard.spec.ts`: keyboard-only flow 5 on desktop.

**Policy**
- `scripts/editor-source-policy.test.ts`:
  - no file in `src/editor/**` over 600 lines, tests excluded;
  - no `\[#[0-9a-fA-F]{3,8}\]`, `white/` or `black/` class fragments in `src/editor/**` or `src/components/ui/**`;
  - no `@tauri-apps/` imports in `src/editor/**`;
  - no `window.prompt(` or `window.confirm(` in `src/**`;
  - no `Codex` or `HyperFrames` in user-visible string literals in `src/editor/**`. Implemented as a JSX text and `aria-label` literal scan with an explicit allowlist for code identifiers.
- `scripts/knip-temporary-block.test.ts`: asserts that the "Editor redesign" temporary block in `knip.jsonc` is absent.

**Editor**
- `src/editor/store/legacy-storage-cleanup.ts`: removes the v1 keys once per profile:
  - `video-creater.workspace-layout.v1`
  - `video-creater:responsive-rail-state`
  - `video-creater.editorTour.dismissed.v1`
  - `video-creater.codexChatTranscript.v1*`
  - `video-creater.timeline-view-state:*`

  The flag is `video-creater.editor.v2.legacy-cleaned`.
- `src/editor/shell/use-safe-area.ts`: CSS `env(safe-area-inset-*)` padding for the top bar and bottom tool bar.

### Modified files

**Editor**
- Mobile components (`mobile-layout.tsx`, `bottom-sheet.tsx`, the timeline touch hook, clip tiles): touch polish.
- `src/lib/runtime/bootstrap.ts`: add marker flags.

**Scripts and gates**
- `scripts/linux-desktop-smoke.mjs`: new selectors.
- `package.json`: add the two policy tests to `test:source-quality`.
- `knip.jsonc`: remove the temporary block.
- `src/vite-build-policy.test.ts`: the no-fixtures assertion.

**Docs**
- `README.md`: add an "Editor" section.
- `docs/product-backlog.md`: VC-001 status; new transitions and editor-redesign foundation rows; VC-006 next action referencing the Captions layout contract.
- `docs/parity.md`: note that the Palmier-layout parity audit is superseded by the 2026-09 redesign for editor chrome.
- `docs/development/linux.md`: smoke steps.
- `docs/development/runtime-and-verification.md`: new e2e specs and policy tests.
- `.agents/skills/video-creater-visuals/SKILL.md`: new layout rules.
- `design-qa.md`: mark the Palmier editor audit historical.

---

### Task 1: Touch and phone polish

- [ ] **Touch interactions:**
  - All touch targets are ≥ 24 px. Clip trim hit areas, keyframe diamonds (via an invisible 24 px hit box) and transition badge edges each get a test asserting the computed hit box size.
  - Long-press (500 ms, cancelled on 8 px movement) opens the context menu on clips, media tiles, cuts and track areas, via a shared `useLongPress` hook in `src/editor/shell/use-long-press.ts`.
  - Sheets have a draggable grab handle. Dragging down more than 80 px closes the sheet. Only one sheet can be open at a time. Opening a sheet while another is open replaces it.
  - The playhead auto-follows during playback on mobile. Manual scroll pauses the follow until playback restarts.
- [ ] **Safe areas:** apply `env(safe-area-inset-top)` / `-bottom` padding to the top bar and bottom tool bar.
- [ ] **Narrow widths:**
  - At 402 px the project name truncates.
  - The tasks pill shows only a spinner and percent.
  - The Export button stays visible.
  - Undo and Redo remain.
  - The gear menu moves into a "More" button when the width is under 380 px.
- [ ] **iPad 820×1180 (mobile layout):** the preview and timeline keep usable heights, and sheets use 55% height when the viewport height exceeds 1000 px.
- [ ] **Tests:**
  - long-press timing and cancel;
  - sheet swipe-to-close;
  - follow-playhead pause;
  - safe-area classes present;
  - top bar at 402 px shows Home, name, Undo, Redo, tasks and Export without overflow. A jsdom width stub is acceptable, but the Playwright test in Task 4 is authoritative.
- [ ] **Commit:** `feat(editor): polish touch interactions, sheets, and safe areas for phones`

### Task 2: Complete fixture transport

- [ ] **Tests per domain module:**
  - **Project store:**
    - revision increments on apply;
    - `save_split_project_to_folder` with a stale `expectedRevision` rejects with the same error shape as Rust (copy the message from `split.rs` `replace_split_project_if_revision`);
    - import adds media with deterministic ids.
  - **Transcription:** the job goes queued, then running, then completed across polls, and the transcript words appear.
  - **Generation:** a completed asset has outputs, and cancel marks it cancelled.
  - **Registration:** `acceptanceFixture` registers every handler, and unknown operations still throw `FixtureOperationUnsupportedError`.
- [ ] **Implement.** The handlers reuse `applyProjectActionsLocally`, `buildCaptionItems` (only where the real backend builds captions; normally captions are built client-side) and `createSampleProject`. Start with sample schema version 2 so split-folder code paths run.
- [ ] **Commit:** `test(runtime): add deterministic fixture handlers for editor acceptance flows`

### Task 3: Policy tests, knip cleanup, legacy storage cleanup

- [ ] **Write `scripts/editor-source-policy.test.ts` and `scripts/knip-temporary-block.test.ts`.** Run them.
  - Expect failures on any real violations. Fix the violations; do not add allowlist entries for them.
  - Delete the temporary knip block. Run `rtk pnpm check:unused`. If knip reports anything, remove the dead export or add its consumer. Do not ignore it.
- [ ] **`legacy-storage-cleanup.ts`:** tests that only listed keys and prefixes are removed, that it runs once, and that v2 keys are untouched. Call it from `EditorRoot` mount.
- [ ] **Add both policy tests to `test:source-quality`** in `package.json`.
- [ ] **Add the no-fixtures-in-production assertion** to `src/vite-build-policy.test.ts`. Build with `rtk pnpm build`, then scan `dist/assets/*.js` for `runtime/fixtures`. Do this only if the existing test pattern supports reading build output. Otherwise assert in `vite.config.ts` that fixtures are imported only behind `import.meta.env.DEV`, via a source scan of `bootstrap.ts`.
- [ ] **Run:** `rtk pnpm test:source-quality && rtk pnpm check:unused && rtk pnpm test`
- [ ] **Commit:** `test(policy): enforce editor source rules and remove the temporary knip block`

### Task 4: Acceptance flows

- [ ] **`e2e/support/editor.ts` helpers**, as listed in the File Map. `openAcceptanceEditor` sets the fixture marker with `acceptanceFixture: true` and routes fixture media.
- [ ] **`e2e/editor-acceptance.spec.ts`:** `for (const viewport of [{ name: "desktop", width: 1440, height: 900 }, { name: "iphone-17-pro", width: 402, height: 874 }])`. The eight spec flows run as separate tests, in order:
  1. **Open and import.** Open the sample project. Media → Import. The fixture adds `preview.webm`. Its tile appears; `+` inserts it and the clip count increases.
  2. **Agent, auto-apply on.** Send "Tighten the pacing". The applied card shows facts. "Show changes" highlights a clip. Undo restores the clip count.
  3. **Agent, review.** Send "Generate three lab shots". The review card appears and the clip count is unchanged. "Generate & place" produces placeholders. Undo removes them. Separately, sending the same prompt and pressing Dismiss leaves the project unchanged.
  4. **Auto-apply off.** Toggle the switch off. The safe prompt shows a review card first.
  5. **Edit a clip.**
     - Select a clip.
     - Set opacity to 50 in Properties (on the phone: Adjust sheet).
     - Add a keyframe with ◇.
     - Split at the playhead (toolbar S on desktop; Clip tools Split on the phone).
     - Undo, then Redo, with ⌘/Ctrl+Z on desktop or the top bar buttons on the phone.
  6. **Captions.**
     - Captions → Generate captions.
     - The fixture transcription completes and the transcript appears.
     - Double-click a word, fix it, press Enter. The caption text on the canvas updates.
     - Styles → choose a preset. All cues change (assert the style class or data attribute on every caption layer).
  7. **Transition.** Effects → Transitions → `+` Crossfade on the selected cut. The badge appears. Properties duration 1.0 s updates the badge label.
  8. **Export.** Export → MP4 · 1080p. Export video. A background task runs and completes. The toast offers "Show in folder", and the reveal call is recorded.
  - **Every test** asserts that `collectConsoleErrors(page)` is empty.
  - **Every test** asserts there is no horizontal document overflow.
- [ ] **`e2e/editor-keyboard.spec.ts`**, desktop, flow 5 using only the keyboard:
  - Tab to the timeline canvas.
  - Arrow to a clip, Enter to select.
  - Tab into Properties, then the opacity numeric field. Type 50 and press Enter.
  - Tab to the keyframe button and press Space.
  - Focus the timeline and press S. Then Mod+Z, then Shift+Mod+Z.
  - Assert visible focus rings with a `:focus-visible` outline check on each focused element.
- [ ] **Run:** `rtk pnpm test:browser`. All specs pass, including the specs from plans 02–08.
- [ ] **Visual check.** Capture each flow's end state at both viewports into `output/editor-acceptance/`. Read every PNG. Record issues and fix them before committing.
- [ ] **Commit:** `test(editor): add redesign acceptance flows at desktop and iPhone 17 Pro sizes`

### Task 5: Linux desktop smoke retarget

- [ ] **Update `scripts/linux-desktop-smoke.mjs` selectors.** The old selectors are at L269–704 (see the plan 02 research inventory):
  - **Empty and sample editor:** `//main[@aria-label='Video editor workspace']`.
  - **Export:** replace `Export project` dialog, `Codec` select and footer Export with:
    - `//button[normalize-space()='Export']`
    - popover format and resolution radio or segmented controls
    - `//button[normalize-space()='Export video']`
  - **Premiere XML and project package:** the popover footer links replace the "More export options" menu.
  - **Transcription:** Captions tab → "Generate captions" replaces Transcript → "Transcribe selected media".
  - **Settings:** gear "Editor menu" → "App settings".
- [ ] **Run it** on an Ubuntu 24.04 host with the packaged or dev app per `docs/development/linux.md`: `rtk pnpm smoke:linux-desktop`. Record the result and the screenshot folder in the commit body.
- [ ] **Commit:** `test(linux): retarget the desktop smoke run to the redesigned editor`

### Task 6: Documentation, backlog, and skill updates

- [ ] **`README.md`.** Add an "Editor" section of 10–20 lines covering:
  - the layout (AI · Media · Audio · Text · Captions · Effects, preview, properties on selection, timeline);
  - agent auto-apply and review behavior;
  - background tasks and export entry points;
  - phone-size layout support (layout only; no phone runtime).
- [ ] **`docs/product-backlog.md`:**
  - **VC-001:** set the status to `implemented`, with next action "Retain packaged native evidence for applied frames, Show changes, and Undo".
  - **VC-006:** the next action references the Captions transcript layout contract (`data-word-index`, `selection.transcriptRange`).
  - **Add rows:**
    - Clip transitions (P1, `implemented`).
    - Native AVFoundation transitions (P2, `idea`).
    - Audio clip speed (P2, `idea`), from the plan 04 deviation.
    - Detach audio (P2, `idea`), if plan 03 omitted it.
  - Update "Last updated".
- [ ] **`docs/parity.md` and `design-qa.md`.** Add a dated note that the editor chrome no longer targets Palmier layout parity. Link the redesign spec.
- [ ] **`docs/development/runtime-and-verification.md`.** List the new e2e specs, the fixture runtime marker flags, and the two policy tests.
- [ ] **`.agents/skills/video-creater-visuals/SKILL.md`.** Rewrite the App Visual Rules table and Timeline Guidance for the redesign:
  - fixed layout;
  - AI tab first;
  - properties only on selection;
  - no timing fields in Properties;
  - tokens only (list the token names);
  - one export entry point;
  - background tasks as the only job surface;
  - phone layout rules (bottom tool bar, sheets, ≥24 px touch targets).

  Keep the Interaction States and Visual QA sections. Update the Visual QA commands to `pnpm test:browser`, and point to `output/editor-acceptance/` captures.
- [ ] **Commit:** `docs(editor): document the redesigned editor and update backlog and visuals skill`

### Task 7: Final verification

- [ ] **Frontend gate:** `rtk pnpm verify:frontend`. Expect PASS on Linux. Record the macOS baseline status from plan 02 Task 4 if it is still pending.
- [ ] **Rust tests:** `rtk cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`. Expect PASS. Also run the default features that include `ges-render`, per `Cargo.toml`.
- [ ] **No legacy editor paths:** `rtk rg -n "components/workspace|EditorWorkspace|modern-editor|responsive-rail|workspace-layout" src scripts e2e docs/development README.md`. Expect no output, apart from historical plan and spec documents under `docs/superpowers/`.
- [ ] **Source sizes:** `rtk node -e "const fs=require('fs'),p=require('path');const walk=d=>fs.readdirSync(d,{withFileTypes:true}).flatMap(e=>e.isDirectory()?walk(p.join(d,e.name)):[p.join(d,e.name)]);const big=walk('src/editor').filter(f=>!/\.test\./.test(f)).map(f=>[f,fs.readFileSync(f,'utf8').split('\n').length]).filter(([,n])=>n>600);console.log(big.length?big:'ok')"`. Expect `ok`.
- [ ] **Worktree check:** confirm `rtk git status --short` is clean after the commits.
- [ ] **Hand-off summary.** Write a short PR description or hand-off note listing:
  - each plan's final commit;
  - the recorded native evidence from plans 06–08;
  - the documented spec deviations: no audio Speed tab, no Reverse unless supported, no Detach audio if absent, AVFoundation falls back to GES for transitions;
  - the pending macOS baseline refresh, if any.

## Acceptance

- All eight spec acceptance flows pass through the UI at 1440×900 and 402×874.
- Flow 5 passes keyboard-only on desktop, and no flow logs console errors.
- The policy tests pass. The temporary knip block is gone, and no editor source file exceeds 600 lines.
- The docs, backlog and visuals skill describe the shipped editor. Evidence boundaries are stated honestly.
