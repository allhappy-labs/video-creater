# Settings Integration and Visual QA Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the four operational Settings slices, remove the placeholder monolith, and prove the final behavior in browser fixtures and a clean packaged macOS app.

**Architecture:** `SettingsShell` owns navigation, snapshot loading, badges, refresh, and operation events. Focused category components own presentation only. Browser fixtures cover deterministic states; packaged Tauri evidence covers native dialogs, processes, Keychain boundaries, model acquisition, and real rendering.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, browser visual QA scripts, Tauri macOS packaging, Rust integration binaries.

## Global Constraints

- Do not delete the old monolith until all six categories have test parity.
- Do not use browser-only mocks as evidence for native runtime readiness.
- Preserve compact flat sections, accessible navigation, and diagnostics
disclosures from the approved design.
- Keep generated evidence out of source folders and downloaded models out of Git.

---

## Task 1: Make `SettingsShell` the only Settings entry point

**Files:**

- Create: `src/components/settings/settings.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/components/settings/settings-shell.tsx`
- Delete after parity: `src/components/settings/model-settings.tsx`
- Modify: all focused category components

- [ ] Add an integration test that renders all six tabs and switches among
General, Models, Agent & MCP, Skills, Storage, and Providers without remounting
the shell.

- [ ] Give `Settings` ownership of:

- initial snapshot load;
- `Check all systems`;
- operation poll/subscription;
- category severity badges;
- active project context;
- preference state;
- action refresh of only the affected category;
- global error boundary with Retry.

- [ ] Keep focused category props typed and free of direct Tauri `invoke`.

- [ ] Move all remaining logic from `model-settings.tsx`, update imports, run
tests, then delete the file only when `rtk rg "model-settings" src` returns no
production imports.

- [ ] Commit:

```bash
rtk git add src/App.tsx src/App.test.tsx src/components/settings
rtk git commit -m "refactor(settings): integrate focused categories"
```

## Task 2: Add deterministic Settings visual fixtures

**Files:**

- Create: `src/lib/settings-visual-qa-fixtures.ts`
- Create: `src/lib/settings-visual-qa-fixtures.test.ts`
- Modify: `src/components/settings/settings.tsx`
- Modify: `scripts/browser-visual-qa.mjs`

- [ ] Add development-only fixture selection with no effect in release builds.

- [ ] Cover:

1. missing transcription model;
2. active model download;
3. failed model download;
4. ready models;
5. GStreamer failed + AVFoundation ready;
6. GStreamer ready + AVFoundation failed;
7. app-server missing;
8. MCP protocol failure;
9. skill checksum mismatch;
10. low storage;
11. provider rejected credential;
12. all systems ready.

- [ ] Use realistic labels and long diagnostic strings to catch overflow.

- [ ] Add screenshot routes/scenarios for each category at desktop and narrow
width.

- [ ] Commit:

```bash
rtk git add src/lib/settings-visual-qa-fixtures* src/components/settings/settings.tsx scripts/browser-visual-qa.mjs
rtk git commit -m "test(settings): add visual state fixtures"
```

## Task 3: Accessibility and interaction verification

**Files:**

- Create: `src/components/settings/settings-accessibility.test.tsx`
- Modify: shared Settings UI components as failures require

- [ ] Test:

- one labeled tablist and one active tabpanel;
- roving arrow-key focus in both orientations;
- action status announced through `aria-live`;
- progress has accessible label/value;
- diagnostics disclosure keyboard operation;
- confirmation focus trap and return focus;
- disabled actions explain why;
- no status communicated by color alone;
- destructive buttons name exact target.

- [ ] Run the test with fake timers to prove polling/event cleanup on unmount.

- [ ] Commit:

```bash
rtk git add src/components/settings
rtk git commit -m "test(settings): verify accessible operations"
```

## Task 4: Add native Settings integration evidence

**Files:**

- Create: `src-tauri/src/bin/video-creater-settings-e2e.rs`
- Add generated report path: `output/settings-readiness/integration/report.json`
- Modify: `package.json`
- Test: Rust tests beside the binary helpers

- [ ] Add:

```json
"e2e:settings": "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-settings-e2e --"
```

- [ ] The binary must use temporary roots and fixture processes to prove:

- snapshot serialization;
- operation journal interruption recovery;
- local-source transcription download/progress/cancel/retry/verify;
- separate transcription and speech readiness;
- render-system health rollup;
- app-server/MCP/validator independent health;
- skill verify and scoped repair;
- storage inventory and cleanup refusal;
- provider health serialization contains no secrets.

- [ ] Emit one JSON report with per-check status, artifact paths, timings, and
diagnostic codes. Exit non-zero on any failed required check.

- [ ] Commit:

```bash
rtk git add src-tauri/src/bin/video-creater-settings-e2e.rs package.json
rtk git commit -m "test(settings): add native readiness evidence"
```

## Task 5: Clean packaged-app acceptance

- [ ] Build the signed app using the production feature list including
`ges-render`.

- [ ] On a clean macOS user account with no model store and no Homebrew runtime
dependency:

1. Open Settings and verify catalog entries render.
2. Download Parakeet from the pinned Hugging Face source.
3. Cancel once, relaunch, confirm interrupted state is not left running, retry,
   and reach Ready.
4. Run transcription helper readiness.
5. Run a real selected-range GStreamer/GES draft with video, audio, and a
   graphics layer.
6. Export H.264 through AVFoundation and WebM through GStreamer.
7. Run app-server, MCP, validator, and skill checks.
8. Choose a suggested project parent.
9. Inspect storage and clear only disposable cache.
10. Exercise a provider Keychain entry without exposing the secret.

- [ ] Capture:

- Settings screenshots;
- operation journal;
- model manifest;
- render reports and logs;
- `otool -L` report;
- runtime manifest hashes;
- signed helper verification;
- final `report.json`.

Store under `output/settings-readiness/packaged-app/`.

## Task 6: Full regression and visual gate

- [ ] Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
rtk pnpm e2e:settings
rtk pnpm visual:qa:browser-release
rtk pnpm verify:gstreamer-runtime
rtk pnpm release:macos
```

- [ ] Inspect every generated Settings screenshot. Reject:

- clipped paths or diagnostics;
- nested card stacks;
- excessive empty space;
- primary actions separated from affected status;
- ambiguous renderer ownership;
- raw internal skill IDs in primary copy;
- copied terminal commands;
- fake update controls;
- missing progress/error recovery.

- [ ] Update the browser baseline only after manual approval of the new images.

## Task 7: Documentation and final review

**Files:**

- Modify: `README.md`
- Modify: `docs/parity.md`
- Modify: the approved design spec only if implementation decisions changed
- Add: `docs/settings-readiness.md`

- [ ] Document:

- where app-managed models are stored;
- pinned source/revision/license diagnostics;
- GStreamer/GES versus AVFoundation responsibilities;
- bundled-runtime verification;
- Settings health/operation command contracts;
- safe cleanup boundaries;
- provider secret non-readback;
- how to run Settings E2E and visual QA.

- [ ] Mark only verified parity items complete in `docs/parity.md`.

- [ ] Use `superpowers:requesting-code-review`, address findings with
`superpowers:receiving-code-review`, then run
`superpowers:verification-before-completion`.

- [ ] Commit:

```bash
rtk git add README.md docs
rtk git commit -m "docs(settings): document operational readiness"
```

