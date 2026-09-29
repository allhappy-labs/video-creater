# Frontend fixes: implementation and validation

Worktree: `.worktrees/frontend-architecture-fixes`
Branch: `fix/frontend-architecture-review`
Starting main: `19b59f347304b38f3a0d4ced29a9496613afe02a`

Fetched upstream main was 34 commits ahead at `32e81fcc`. It was integrated before substantial implementation (`735c24bf`), preserving its runtime transport and browser-QA changes. The refreshed baseline passed 114 files / 2,120 tests in 96.12 seconds and all 17 verification-policy tests.

## Scope tracking

The approved source is [the frontend architecture review](2026-09-11-frontend-architecture-review.md). This matrix tracks both accepted defects and lower-priority improvements; evidence is completed as changes are verified.

| Finding | Owner | Status |
| --- | --- | --- |
| Overlapping project opens | State | Implemented `d524ce08`; 48 focused tests independently passed; Sol cross-review found no actionable defect |
| Project identity/session isolation | State | Implemented `d524ce08`; 48 focused tests independently passed; Sol cross-review found no actionable defect |
| Native menu active-view ownership | State | Implemented `d524ce08`; 48 focused tests independently passed; Sol cross-review found no actionable defect |
| Settings request supersession | Interactions | Implemented in `b9ec9e12`; 348 focused tests independently passed; review corrections completed |
| Source/timeline keyboard conflict | Interactions | Implemented in `b9ec9e12`; 348 focused tests independently passed; review corrections completed |
| Generation polling lifetime | State | Implemented `d524ce08`; 48 focused tests independently passed; Sol cross-review found no actionable defect |
| Preferences bootstrap Retry | State | Implemented `d524ce08`; 48 focused tests independently passed; Sol cross-review found no actionable defect |
| Viewer tabs keyboard semantics | Interactions | Implemented in `b9ec9e12`; 348 focused tests independently passed; review corrections completed |
| Canonical frame coverage recovery | Interactions | Implemented in `b9ec9e12`; 348 focused tests independently passed; review corrections completed |
| Media transport versus clock sync | Interactions | Implemented in `b9ec9e12`; 348 focused tests independently passed; review corrections completed |
| Decorative timeline accessibility | Interactions | Implemented in `b9ec9e12`; 348 focused tests independently passed; review corrections completed |
| Inspector dirty drafts/conflicts | Inspector | Implemented `83876190`, corrected `7c8ff566`; action-scoped draft reconciliation and resize cleanup verified |
| Inspector resize coalescing | Inspector | Implemented `83876190`, corrected `7c8ff566`; action-scoped draft reconciliation and resize cleanup verified |
| Root render-error recovery | State | Implemented `d524ce08`; 48 focused tests independently passed; Sol cross-review found no actionable defect |
| Feature loading / acceptance runner split | State | Implemented `d524ce08`; 48 focused tests independently passed; Sol cross-review found no actionable defect |
| Focused ownership extraction | State / Interactions / Inspector | Session key, disposable poller, draft lifecycle and active-view ownership extracted; focused refactoring rather than a component rewrite |
| Knip files, dependencies, functions, components and exports | Tooling | Implemented `05392794`; functional browser checks and all 82 fresh Linux baseline comparisons passed |
| Repo-local Playwright and browser regression checks | Tooling | Implemented `05392794`; functional browser checks and all 82 fresh Linux baseline comparisons passed |

## Fresh baseline

- `pnpm install --frozen-lockfile`: passed.
- `pnpm exec vitest run --maxWorkers=2`: 106 files, 2,085 tests passed in 86.20 seconds before edits.
- `pnpm check:source-quality`: passed.
- `pnpm check:tooling-source`: passed for 42 files.

## Verification limits

This work runs on Ubuntu. Browser fixtures exercise real React UI with a simulated Tauri bridge. Native macOS packaging, menu rendering and media sidecars need native verification; browser/unit results do not claim it. Performance changes will be described by measured behavior or bundle output, not assumed target-device speedups.

## Review follow-ups

- Sol cross-review found failed prepared frames could not retry unchanged URLs and produced duplicate error surfaces. Fixed in `e1ae6384`; PreviewPanel 71 tests passed.
- Astra's final focused review requested action-scoped draft reconciliation, selected-tab focus after closing a middle source, and owned Vite readiness. These are tracked in [the Astra report](frontend-fixes-astra-review.md).
- An integrated run passed 2,139 tests and failed two later workspace interactions. Root traced both failures to the newly added resize test retaining animation-frame mocks, rather than accepting isolated retries as a passing suite. Scoped mock cleanup in `7c8ff566` fixed the leak; a fresh full run passed all 2,145 tests in 117 files (106.10 seconds).

## Separate unused-code cleanup

Removed dead card/provider-account modules, the unused RenderQualityControl and test-only SkillsSettings components, three uncalled bridge helpers, and two unused dependencies (`@dnd-kit/core`, `dnd-timeline`). Retained active skill verification coverage and locally used types, making unused exports private. Knip exceptions cover convention-loaded PostCSS/autoprefixer and host tools, not frontend source. See [cleanup and browser tooling details](frontend-fixes-tooling.md). `npx knip --no-progress` completed with exit 0 and no findings after cleanup.

## Build observations

Production build passed. The app entry is 75.03 kB (22.51 kB gzip); Settings, project settings, system health, acceptance runner and editor load as separate chunks. The editor chunk remains 776.94 kB (201.98 kB gzip), producing Vite's size warning. This verifies deferred loading, not a measured device-speed improvement or completion of a broad editor rewrite.

## Integrated verification

- `pnpm exec vitest run --maxWorkers=2`: 117 files, 2,145 tests passed; no unhandled errors.
- `pnpm lint`: passed.
- `pnpm check:source-quality`: passed.
- `pnpm check:tooling-source`: 62 files passed.
- `pnpm test:source-quality`: 38 tests passed, including Linux baseline selection and strict threshold contracts.
- `pnpm test:verification-policy`: 17 tests passed.
- `npx knip --no-progress`: passed, no findings.
- `pnpm build`: passed, with the documented editor chunk-size warning.

`pnpm test:browser` passed (3 adapter tests, 2 real-app tests). `pnpm visual:qa:browser-release` passed all 82 fresh Linux screenshot comparisons, with no missing, extra or duplicate frames and passing manifest integrity/platform checks.

## Browser visual review and baseline provenance

The first complete run captured all 82 scenarios, then failed strict comparison against every original baseline (mismatch range 1.16–47.54%). This result was not accepted as a pass. Root inspected all captures in 11 contact sheets and full-size before/after Home narrow, modern-editor-narrow and timeline-focus-desktop, plus desktop/narrow app smoke. The original narrow-editor baseline itself has no visible preview; the corrected image has a fitted preview and timeline. The original desktop image places default media off-center; full-size media containment now centers it. Font metrics also differ across the baseline environments: this VM resolves both Inter and sans-serif to DejaVu Sans, changing Home wrapping.

A separate reviewed Linux baseline was added while the original PNGs/manifest remain intact. Pixel and channel thresholds remain 0.01 and 4. A new full capture, rather than comparing the candidate files to themselves, passed all 82 comparisons and manifest checks. The reviewed Linux environment is Ubuntu 24.04.4 x64, Playwright 1.63.0, Chromium 153.0.8010.12, DejaVu Sans. This is a layout-level screenshot review backed by functional assertions, not exhaustive manual interaction or native media verification.

After the final two media-fit class changes, root independently reran all 44 compositor tests and the production build successfully. Frozen lockfile installation also passed. Astra's bounded follow-up approved its three corrected findings; [review dispositions](frontend-fixes-astra-review.md#follow-up-disposition).

## Completion

All accepted findings and scoped improvements from the frontend review are implemented. Sol medium agents performed implementation and cross-checks; Astra performed a focused final review and a bounded follow-up approving the three corrected findings. Root independently ran the integrated unit, build, policy, unused-code and targeted regression checks and reviewed screenshot evidence. The final Linux baseline repeat passed at unchanged thresholds.

The original baseline files remain preserved for their existing platform workflow; macOS pixels, native menus, media sidecars and packaged-app behavior remain unverified here. The editor bundle warning is the remaining measured optimization opportunity, not a failed build.
