# Frontend cleanup and browser QA

Date: 2026-09-11

## Result

The frontend now has a pinned repository-local unused-code gate and Playwright browser gate. The browser tests use the explicit fixture runtime, start an isolated strict-port Vite server, exercise Home, Settings, and the real editor at desktop and narrow sizes, and fail on uncaught page or console errors. The editor assertions also require a visible preview, loaded video metadata, nonzero video geometry, and no `Preview failed` state.

The legacy screenshot suite now uses the repository Playwright adapter instead of a wrapper under a user's home directory. Its default command starts and cleans its own Vite server at `127.0.0.1:4179`; `--url` and `VISUAL_QA_BASE_URL` remain deliberate overrides for an existing target. The adapter binds its authenticated session daemon to loopback, scopes session state by checkout and session, preserves the page across commands, removes state on close, and cleans browser/Vite processes on normal exit and signals.

## Unused-code cleanup

The original Knip report identified three unused files, two unused production dependencies, one convention-loaded development dependency, host-tool binaries, and unused exports/types. The implementation:

- removed `@dnd-kit/core` and `dnd-timeline`;
- removed unused `card`, `provider-account`, and `render-quality-control` modules;
- removed the test-only `SkillsSettings` component after confirming `ProjectSettings` retains the active skill verification and repair coverage;
- removed the unused `cancelModelDownload`, `captureCanonicalPreviewFrameInSplitProjectFolder`, and `renderWebmToSplitProjectFolder` functions;
- made module-internal declarations private rather than deleting types still used inside their declaring modules;
- consolidated the duplicate app-preferences default under `defaultAppPreferences`;
- added `knip.jsonc` without suppressing frontend source or disabling unused export/type analysis.

The Knip exclusions are bounded to actual tool conventions:

- `postcss.config.js` is loaded by Vite/PostCSS convention;
- `autoprefixer` is resolved from a string-key PostCSS plugin declaration;
- `brew`, `cc`, `ffmpeg`, `gst-inspect-1.0`, `pkg-config`, `rustc`, and `swift` are host prerequisites rather than npm dependencies;
- `scripts/playwright-cli.mjs` is a dynamic command entry point.

Run the gate with:

```sh
pnpm check:unused
```

## Browser evidence

Install the pinned Chromium revision with:

```sh
pnpm playwright:install
```

Run both the adapter lifecycle regression and real-app browser smoke with:

```sh
pnpm test:browser
```

The successful run produced:

- 3 adapter contract/lifecycle tests;
- 2 real-app Playwright tests;
- desktop screenshots at 1440 × 960;
- narrow Settings/Home screenshots at 390 × 844 and a supported editor screenshot at 900 × 900.

Artifacts are written beneath `output/playwright/app-smoke/`. Playwright retains traces and failure screenshots when a browser test fails.

The repository visual runner also passed a self-hosted `home::desktop` regression and cleaned port 4179 afterward. The first stable release run captured all 82 scenarios and proved that the original baseline, from a different platform, did not match Linux: 0 of 82 frames were within the 1% threshold, with mismatch ratios from 1.16% to 47.54%. Root reviewed all 82 Linux captures through 11 contact sheets and full-size representative comparisons, including Home narrow, the restored single-pane preview, desktop timeline focus, and real-app smoke images. The differences matched Linux font metrics and the intended preview/layout fixes; no additional layout defect was found.

The approved captures are stored separately under `docs/visual-qa/browser-visual-baseline-linux-x64`; the original baseline and manifest are unchanged. A platform wrapper selects this baseline and its integrity manifest only on Linux x64, retaining the original paths elsewhere and retaining the 1% / channel-delta 4 thresholds. A fresh second 82-shot run passed with zero mismatches, complete manifest integrity, and no missing, extra, or duplicate frames. The exact passing comparison is `output/playwright/browser-visual-qa/browser-visual-baseline-comparison.json`.

The Linux manifest records Ubuntu 24.04.4 x64, Playwright 1.63.0, Chromium 153.0.8010.12, and the host font resolution (`Inter` and `sans-serif` both resolve to DejaVu Sans Book). It assumes system fonts on the verification host and does not install external fonts.

## Verification

Passed on this worktree:

- `pnpm test:browser` — 3 adapter tests and 2 app smoke tests;
- `pnpm exec vitest run src/components/workspace/timeline-preview-compositor.test.tsx` — 44 tests;
- `pnpm exec vitest run src/browser-visual-qa-script.test.ts` — 4 tests, including occupied-port refusal;
- focused Vitest coverage — 79 tests across browser tooling, fixture transport, app settings, and Settings integration/accessibility/model ownership;
- `pnpm check:unused` — no diagnostics;
- `pnpm lint`;
- `pnpm check:source-quality`;
- `pnpm check:tooling-source` — 62 files in the final integrated run;
- `pnpm test:source-quality` — 38 tests;
- `pnpm visual:qa:browser -- --only home::desktop --out output/playwright/self-hosted-home`;
- `pnpm visual:qa:browser-release` — 82 fresh comparisons passed with zero mismatches;
- `git diff --check`.

`pnpm check:unused` passes with no diagnostics after the final four inspector declarations were made module-private. `pnpm lint` also passes. The real-app smoke requires a visible preview viewport, loaded video/audio fixture media, nonzero painted media, and full containment of the active video inside the Fit viewport. The strict Linux x64 release baseline gate passes after explicit visual review; no comparison threshold was suppressed.

No macOS, packaged-app, signing, or physical-device verification was performed on this Ubuntu VM.
