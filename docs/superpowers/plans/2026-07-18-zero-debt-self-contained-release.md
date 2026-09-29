# Zero-Debt Self-Contained Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Video Creater release self-contained and zero-debt: strict TypeScript, clean Rust formatting and Clippy, deterministic 73/73 visual QA, passing native macOS integration tests, and a freshly signed/notarized app and DMG whose packaged runtime works without package-manager state.

**Architecture:** Add executable policy gates first, then repair each failing subsystem at its source. Keep release runtime classification explicit: app-relative bundled artifacts, supported absolute macOS system facilities, or development-only tooling that is absent from release behavior. Preserve dotLottie's fail-closed feature policy, use the reviewed production Rust feature set for Clippy, and verify native/media behavior outside the restricted sandbox. Finish by building and exercising the signed package under an isolated home and sanitized `PATH`.

**Tech Stack:** Tauri 2, Rust/Cargo/Clippy/rustfmt, React 19, TypeScript 5.7, Vite 6, Vitest, Node test runner, Playwright-based visual QA, GStreamer/GES 1.28.2, macOS AVFoundation/Metal, Developer ID signing and Apple notarization.

## Global Constraints

- Work directly on `main`, as authorized, and use Conventional Commits.
- Prefix shell commands with `rtk`; use `apply_patch` for source edits.
- Do not stage `.superpowers/sdd/progress.md`.
- Do not overwrite unknown user changes. Review the current partially formatted Rust diff with whitespace ignored before accepting it.
- Implement every behavior change test-first. Record the expected red result before changing production code.
- Do not use crate-wide or workspace-wide Clippy suppression. A retained structural lint uses the narrowest `#[expect(clippy::lint_name, reason = "the exact architectural constraint")]` and has a behavior test.
- Do not weaken TypeScript with project-wide `any`, blanket assertions, `@ts-ignore`, or disabled strict flags.
- Do not classify build/test commands as installed-app dependencies. The release gate scans packaged behavior and release-facing recovery text separately from developer tooling.
- Do not update visual baselines until two same-commit fresh captures match one another.
- Do not call the release complete until every gate is freshly green from the final source commit and the retained app/DMG evidence says `passed` without a debt exception.

---

## Task 1: Establish executable zero-debt source policies

**Files:**

- Create: `scripts/source-quality-policy.mjs`
- Create: `scripts/source-quality-policy.test.ts`
- Modify: `package.json`
- Modify: `tsconfig.json`
- Modify: `tsconfig.node.json`

- [ ] **Step 1: Write failing policy tests**

Add Node tests that run the policy against temporary fixtures and the repository. Cover:

Name the cases exactly:

```text
rejects JavaScript inside the frontend source boundary
requires every strict TypeScript compiler option
rejects broad Rust Clippy allow attributes
requires a reason on every Clippy expect attribute
rejects forbidden bare commands in release Rust targets
permits package-manager commands in explicitly classified build tooling
```

The frontend scan must reject `src/**/*.js` and `src/**/*.jsx`. The compiler assertion must require `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, and `noImplicitOverride` in both TypeScript configurations. The Rust annotation scan must reject `#![allow(clippy::...)]`, `#[allow(clippy::...)]`, and `#[expect(clippy::...)]` without a non-empty `reason`.

The runtime scan must inspect release-reachable Rust source (`src-tauri/src`, excluding evidence-only binaries explicitly listed in the policy) for `Command::new` or displayed recovery commands using bare `pnpm`, `npm`, `node`, `brew`, `gst-launch-1.0`, `ffmpeg`, `temporal`, or `protoc`.

- [ ] **Step 2: Prove the policy is red on the current tree**

Run:

```bash
rtk node --test scripts/source-quality-policy.test.ts
```

Expected: failures identify missing strict flags, current release `pnpm`/Homebrew recovery paths, and any existing broad Clippy exceptions.

- [ ] **Step 3: Implement the policy scanner and canonical scripts**

Implement deterministic recursive walking with sorted paths and precise `path:line` diagnostics. Add:

```json
"check:source-quality": "node scripts/source-quality-policy.mjs",
"test:source-quality": "node --test scripts/source-quality-policy.test.ts",
"rust:fmt": "cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check",
"rust:clippy": "cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings",
"check:tooling-source": "node scripts/check-tooling-source.mjs"
```

Keep the reviewed default production feature set in `rust:clippy`; do not add `--all-features` because dotLottie deliberately rejects unsupported renderer features.

- [ ] **Step 4: Add the tooling syntax boundary**

Create `scripts/check-tooling-source.mjs` to sort every repository-owned `.mjs`/`.js` build, release, and evidence script outside `src/`, run Node's syntax checker, and aggregate failures. Exclude generated output, `node_modules`, Cargo targets, and vendored third-party source. Test the checker with one valid and one invalid temporary script.

- [ ] **Step 5: Verify and commit the policy harness**

Run:

```bash
rtk node --test scripts/source-quality-policy.test.ts
rtk node scripts/check-tooling-source.mjs
```

Expected: fixture tests pass; repository policy remains red only for the production issues intentionally fixed in Tasks 2-4.

Commit:

```bash
rtk git add package.json scripts/source-quality-policy.mjs scripts/source-quality-policy.test.ts scripts/check-tooling-source.mjs tsconfig.json tsconfig.node.json
rtk git commit -m "test(readiness): enforce zero-debt source policies"
```

---

## Task 2: Make the complete Rust workspace format-safe

**Files:**

- Modify: `src-tauri/vendor/dotlottie-rs/src/lib.rs`
- Create only if rustfmt requires them: feature-gated stub module files under `src-tauri/vendor/dotlottie-rs/src/`
- Review/modify: the existing partially formatted files reported by `git status`
- Modify: `src-tauri/tests/one_click_edit.rs`
- Modify: `src-tauri/src/transcription/store.rs`
- Test: `scripts/source-quality-policy.test.ts`

- [ ] **Step 1: Capture the rustfmt traversal failure**

Run:

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
```

Expected red: rustfmt cannot resolve absent dotLottie modules such as `src/audio.rs`; retain the exact missing-module list.

- [ ] **Step 2: Add a fail-closed vendor feature regression**

Extend the policy test to assert each unsupported dotLottie feature still reaches an explicit `compile_error!`, and the production Cargo feature set does not enable it. If rustfmt requires module files, make them non-functional stubs guarded by the same unsupported-feature compile error; do not restore renderer implementations.

- [ ] **Step 3: Make dotLottie traversable without enabling capabilities**

Adjust module declarations/stubs so rustfmt can parse the complete workspace while Cargo still fails closed for unsupported dotLottie features. Prove both boundaries:

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
rtk cargo check --manifest-path src-tauri/Cargo.toml --all-features
```

Expected: rustfmt reaches all files; the all-features check fails with the intentional, named dotLottie unsupported-feature error rather than a missing-file error.

- [ ] **Step 4: Review and accept only intentional formatting changes**

Inspect current dirty Rust files with:

```bash
rtk git diff --check
rtk git diff -w -- src-tauri/src src-tauri/tests
```

Separate semantic differences from formatting. Preserve semantic user changes, format the workspace once, and review the resulting diff. Remove `ready_manifest_for_entry` only after confirming it has no call sites; remove the unused `RenderQualityProfile` import.

- [ ] **Step 5: Verify and commit formatting**

Run:

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
rtk cargo check --manifest-path src-tauri/Cargo.toml --workspace --all-targets
```

Expected: both pass with no warnings.

Commit all reviewed formatting/vendor traversal changes, excluding controller state:

```bash
rtk git add src-tauri
rtk git restore --staged .superpowers/sdd/progress.md
rtk git commit -m "style(rust): make the workspace fully format-safe"
```

---

## Task 3: Strengthen TypeScript strictness and repair every type error

**Files:**

- Modify: `tsconfig.json`
- Modify: `tsconfig.node.json`
- Modify: affected `src/**/*.ts` and `src/**/*.tsx` files reported by `tsc`
- Modify: focused neighboring `*.test.ts`/`*.test.tsx` files
- Test: `scripts/source-quality-policy.test.ts`

- [ ] **Step 1: Turn on the agreed strict flags**

Add to both compiler option blocks:

```json
"noUncheckedIndexedAccess": true,
"exactOptionalPropertyTypes": true,
"noImplicitOverride": true
```

- [ ] **Step 2: Capture and classify the red compiler output**

Run:

```bash
rtk pnpm lint
```

Group errors by root cause: unchecked indexed lookup, optional property construction, subclass override, and unvalidated external input. Save the command output in the task log, not the repository.

- [ ] **Step 3: Fix unchecked access at the source**

For arrays/maps, use an explicit guard or domain-safe helper rather than `!`:

```ts
const clip = clips[index];
if (!clip) return undefined;
return buildSelection(clip);
```

For required invariant violations, throw a domain-specific error with the missing identifier. Add or update focused tests for empty lists, missing IDs, and first/last boundary indices.

- [ ] **Step 4: Fix exact optional properties**

Omit absent keys instead of assigning `undefined`:

```ts
const request = {
  projectId,
  ...(modelId === undefined ? {} : { modelId }),
};
```

At IPC/provider/JSON boundaries, validate unknown values before constructing typed domain objects. Do not silence failures with broad casts.

- [ ] **Step 5: Fix override declarations and run focused tests**

Add `override` only where TypeScript proves inheritance. Run affected Vitest files after each source cluster:

```bash
rtk pnpm test
```

- [ ] **Step 6: Prove strict frontend compliance**

Run:

```bash
rtk pnpm lint
rtk pnpm test:source-quality
rtk pnpm check:tooling-source
rtk pnpm test
rtk pnpm build
```

Expected: all pass; `src/` contains only `.ts`/`.tsx`; no new ignore/blanket-assertion escape hatch exists.

Commit:

```bash
rtk git add tsconfig.json tsconfig.node.json src scripts/source-quality-policy.test.ts
rtk git commit -m "refactor(frontend): enforce complete strict TypeScript"
```

---

## Task 4: Eliminate actionable Clippy findings with rare, justified expectations

**Files:**

- Modify: Rust sources and tests reported by the canonical Clippy command
- Modify: `scripts/source-quality-policy.mjs`
- Test: focused Rust unit/integration tests adjacent to each change

- [ ] **Step 1: Record the canonical red Clippy inventory**

Run:

```bash
rtk cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings
```

Expected red categories include dead code, redundant closures/borrows, needless `Ok(...?)`, unnecessary unwraps, backward iteration via `.last()`, collapsible conditions, manual range/contains/is-multiple-of operations, derivable defaults, needless range loops, `items_after_test_module`, large result errors, large enum variants, and high-argument functions.

- [ ] **Step 2: Fix mechanical correctness and clarity findings**

Apply semantic-preserving standard-library forms (`next_back`, `RangeInclusive::contains`, slice `contains`, `is_multiple_of`, `#[derive(Default)]`, direct function references). Run the nearest test target after each file cluster, then rerun Clippy to shrink the inventory.

- [ ] **Step 3: Fix control-flow and ownership findings**

Collapse duplicate branches, replace `unwrap` after a condition with `let Some(...) = ... else`, remove needless clones/borrows/question marks, and move declarations before test modules. Add regression tests where the old shape masked an empty/missing/error branch.

- [ ] **Step 4: Refactor large domain boundaries where it improves the model**

Box genuinely large enum payloads and introduce focused parameter structs when callers already pass a cohesive request. Update serialization and versioned-contract tests before changing public shapes.

- [ ] **Step 5: Audit structural exceptions one at a time**

For a versioned IPC/error/protocol type where boxing or signature reshaping would harm the contract, add only:

```rust
#[expect(
    clippy::result_large_err,
    reason = "the versioned diagnostic error preserves structured render evidence across IPC"
)]
```

Use the exact real architectural reason and attach it to the narrowest item. Add a behavior/serialization test. The source policy must reject broad `allow`, missing reasons, and crate-level Clippy expectations.

- [ ] **Step 6: Verify and commit in bounded clusters**

After each coherent cluster:

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
rtk cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings
rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1
```

Use commits such as:

```bash
rtk git commit -m "refactor(rust): resolve actionable clippy findings"
rtk git commit -m "refactor(render): narrow validated lint exceptions"
```

Final expected result: canonical Clippy exits zero and the policy inventory contains only item-scoped, reasoned expectations backed by tests.

---

## Task 5: Remove package-manager and bare-tool dependencies from release behavior

**Files:**

- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/src/render_pipeline/project_export.rs`
- Modify: `src-tauri/src/render_pipeline/report.rs`
- Modify: `src-tauri/src/search/visual_cache.rs`
- Modify: `src-tauri/src/media_inspection.rs`
- Modify: `src-tauri/src/audio_sync.rs`
- Modify: `src-tauri/src/precompose/audio_denoise.rs`
- Modify: `src-tauri/src/precompose/intermediate.rs`
- Modify: `src/components/workspace/render-report-panel.tsx`
- Modify: Settings/workflow/render UI tests containing `brew install` or `pnpm visual:qa`
- Create: `scripts/release-runtime-policy.mjs`
- Create: `scripts/release-runtime-policy.test.ts`
- Modify: release build/verification scripts and `package.json`

- [ ] **Step 1: Add failing release-runtime classification tests**

Test three independent layers:

1. release source has no bare forbidden `Command::new` invocation;
2. release-facing payloads and UI have no package-manager install/QA commands;
3. a packaged fixture fails if a Mach-O dependency escapes the bundle/system allowlist or if a required executable is unclassified.

Build/test/evidence-only paths must be explicitly listed by path and purpose, not skipped by a broad directory regex.

- [ ] **Step 2: Prove current failures**

Run:

```bash
rtk node --test scripts/release-runtime-policy.test.ts
rtk node scripts/release-runtime-policy.mjs --source
```

Expected red: `main.rs` invokes `pnpm`; workflow preflight emits `brew install protobuf` and `brew install temporal`; render review exposes a pnpm QA command; release modules call bare `ffmpeg`/`gst-launch-1.0`.

- [ ] **Step 3: Replace the preview-QA package-manager path**

Move preview review execution behind a Rust/Tauri command that directly invokes the bundled/native implementation, returning structured evidence fields rather than a shell command string. The UI should offer **Run review**, **Retry**, and **Reveal evidence**, not copy a `pnpm` command. Update render report serialization and tests.

- [ ] **Step 4: Make Temporal/protobuf development-only or bundled**

If the installed app does not require the Temporal worker, classify its preflight as development-only and remove the tools/install hints from production workflow status and UI. Keep developer verification in package scripts/docs. If a release path really requires either binary, bundle and sign it with a closed dependency inventory instead; do not add Homebrew recovery instructions.

- [ ] **Step 5: Route media tools through bundled/system resolvers**

For production `ffmpeg`/`gst-launch-1.0` calls, prefer existing Rust/GStreamer APIs. Where a sidecar is still required, resolve an app-relative absolute helper from the resource directory and verify it during packaging. Tests may use developer binaries only inside explicit test helpers. No release call may rely on `PATH`.

- [ ] **Step 6: Add recursive packaged-runtime inspection**

The verifier must:

- enumerate all Mach-O executables, dylibs, frameworks, GStreamer plugins, and helpers inside the app;
- parse `otool -L` and `otool -l` results;
- resolve `@rpath`, `@loader_path`, and `@executable_path` against the bundle;
- permit only resolved in-bundle paths, `/usr/lib`, and `/System/Library`;
- reject `/opt/homebrew`, `/usr/local`, `node_modules`, checkout paths, unresolved dependencies, unsigned nested code, and unclassified executables;
- verify GStreamer plugin scanner and plugin inventory, GES/GStreamer versions, manifests, provenance, and licenses.

- [ ] **Step 7: Verify source and focused behavior**

Run:

```bash
rtk pnpm test:source-quality
rtk node --test scripts/release-runtime-policy.test.ts
rtk node scripts/release-runtime-policy.mjs --source
rtk pnpm test -- src/components/workspace/render-report-panel.test.tsx src/components/workspace/editor-workspace.test.tsx src/lib/project.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml render_pipeline -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml temporal_workflows -- --test-threads=1
```

Expected: all pass, and source scans distinguish legitimate developer tooling from installed-app behavior.

Commit:

```bash
rtk git add src src-tauri scripts package.json
rtk git commit -m "fix(runtime): remove package-manager release dependencies"
```

---

## Task 6: Make native macOS integration verification canonical

**Files:**

- Create: `scripts/run-native-rust-tests.mjs`
- Create: `scripts/native-rust-test-policy.test.ts`
- Modify: `package.json`
- Modify: affected native tests only if they incorrectly skip supported macOS
- Modify: release report generator

- [ ] **Step 1: Add the runner contract tests**

Assert the runner executes the full serial Cargo suite, records native environment metadata, refuses a report containing failed/skipped-required tests, and identifies the required Metal, `afconvert`, bundled GStreamer filmstrip, Keychain, and notification lanes.

- [ ] **Step 2: Demonstrate sandbox/native boundary**

Run the three known tests inside the restricted sandbox and capture their environmental failures. Then run the exact same tests with native macOS access:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml gpu_matches_canonical_cpu_blends_with_documented_tolerance -- --exact --nocapture --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml system_afconvert_decodes_the_retained_mp4_speech_fixture -- --exact --nocapture --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml bundled_decoder_extracts_ordered_distinct_frames_from_sample_media -- --exact --nocapture --test-threads=1
```

Expected with native access: all pass unchanged.

- [ ] **Step 3: Implement the canonical native runner/report**

Run the entire Rust suite serially with native access and emit machine-readable results that separate unit and native integration lanes. Platform guards may skip only unsupported platforms; supported macOS must execute required lanes.

- [ ] **Step 4: Run the complete native suite**

```bash
rtk pnpm rust:test:native
```

Expected: all non-ignored tests pass; the single intentional ignored test is named and justified; no sandbox failure is converted into a pass.

Commit:

```bash
rtk git add scripts/run-native-rust-tests.mjs scripts/native-rust-test-policy.test.ts package.json
rtk git commit -m "test(native): require full macOS integration coverage"
```

---

## Task 7: Repair visual determinism and approve the 15 changed baselines

**Required skills:** `video-creater-visuals`, `superpowers:systematic-debugging`, and `superpowers:test-driven-development`.

**Files:**

- Modify if nondeterministic: `scripts/browser-visual-qa.mjs` and affected scenario fixtures/components
- Modify after review only: 15 images under `docs/visual-qa/browser-visual-baseline/`
- Create/update: visual comparison evidence under the final release output directory

- [ ] **Step 1: Launch one clean Vite server and capture twice**

Use two empty output directories from the same source commit. Wait for fonts, animations, media metadata, and scenario readiness exactly as the harness specifies.

- [ ] **Step 2: Compare fresh capture A to fresh capture B**

Run the visual comparator with the release thresholds. Expected: 73/73 fresh-to-fresh matches. If any mismatch exists, do not touch baselines.

- [ ] **Step 3: Debug nondeterminism before baseline work**

For each mismatch, isolate state/time/font/media/animation readiness, write a scenario regression, make the smallest deterministic fix, and repeat both complete captures until fresh-to-fresh is 73/73.

- [ ] **Step 4: Review every existing-baseline mismatch**

Inspect baseline/current/diff for these known scenarios:

```text
timeline-resize-narrow
timeline-drag-narrow
preview-narrow
preview-caption-phone-narrow
editor-palmier-desktop
editor-pane-budget-laptop
editor-pane-budget-compact-desktop
generation-attached-desktop
media-overflow-desktop
captions-workbench-desktop
editor-speech-details-desktop
inspector-details-desktop
inspector-ai-edit-desktop
agent-starters-desktop
profile-menu-desktop
```

Approve only output that preserves responsive containment, timeline usability, intended interaction state, readable hierarchy, and accessible focus/contrast. Fix product regressions instead of blessing them.

- [ ] **Step 5: Update approved baselines and prove the release suite**

Run:

```bash
rtk pnpm visual:qa:browser-release
```

Expected: 73/73 scenarios pass, including Settings 2/2 and modern-editor 34/34.

Commit:

```bash
rtk git add docs/visual-qa/browser-visual-baseline scripts/browser-visual-qa.mjs src
rtk git commit -m "test(visual): approve deterministic editor baselines"
```

Stop the Vite/browser processes and prove no task-owned process remains.

---

## Task 8: Clean generated runtime candidates and prove repository-wide gates

**Files:**

- Inspect: `src-tauri/resources/render-runtime.candidate-51493/`
- Compare: published runtime source used by `src-tauri/tauri.conf.json`
- Modify: `package.json` verification composition
- Modify: readiness documentation/report templates

- [ ] **Step 1: Compare the candidate with the published runtime**

Compare manifests, hashes, GStreamer/GES versions, plugin sets, helpers, licenses, and provenance. If the directory is an obsolete task-generated duplicate, remove it; if provenance is unknown, preserve and report it rather than deleting user data.

- [ ] **Step 2: Compose the zero-debt repository gate**

Ensure the canonical verification runs, in order:

```bash
rtk pnpm check:source-quality
rtk pnpm check:tooling-source
rtk pnpm lint
rtk pnpm test
rtk pnpm build
rtk pnpm rust:fmt
rtk pnpm rust:clippy
rtk pnpm rust:test:native
rtk pnpm visual:qa:browser-release
```

- [ ] **Step 3: Run the complete gate from a clean task-owned source state**

Expected: every command exits zero. `.superpowers/sdd/progress.md` may remain controller-owned and dirty, but the release builder must record and exclude it under the explicit dirty-state policy; no task-owned source or baseline change may remain uncommitted.

- [ ] **Step 4: Commit gate composition and hygiene changes**

```bash
rtk git add package.json scripts docs src-tauri/resources
rtk git restore --staged .superpowers/sdd/progress.md
rtk git commit -m "build(readiness): compose zero-debt release gates"
```

---

## Task 9: Build, sign, notarize, and accept the final self-contained release

**Files:**

- Modify if a gate finds a defect: release build/verification scripts
- Create: retained evidence under the release script's commit-keyed `output/settings-readiness/final-gate/release-<short-source-commit>/` directory
- Modify: final readiness report and canonical readiness documentation

- [ ] **Step 1: Run the literal release command from the final source commit**

Record `git rev-parse HEAD`, exact command, environment classification, and dirty-state policy. Build fresh helpers, GStreamer/GES runtime, app, and DMG; do not reuse the earlier `138b1ae9` artifact as final proof.

- [ ] **Step 2: Verify signing and recursive dependency closure**

Require Developer ID identity, hardened runtime, secure timestamps, valid nested signatures, complete Mach-O closure, in-bundle RPATH resolution, runtime inventory, license/provenance inventory, and absence of forbidden prefixes/commands.

- [ ] **Step 3: Notarize and staple both deliverables**

Submit the app archive and DMG, wait for Apple `Accepted`, record notarization identifiers, staple, validate staples, and require Gatekeeper acceptance after stapling. Record SHA-256 hashes and sizes.

- [ ] **Step 4: Run packaged acceptance under an isolated environment**

Launch the signed app with a temporary isolated `HOME` and a `PATH` containing only required macOS system directories. Exercise:

- Settings navigation and real status for General, Models, Agent & MCP, Skills, Storage, and Providers;
- model download, progress, cancellation/retry, verification, activation, reveal, and deletion;
- bundled GStreamer/GES composition and supported export delivery;
- provider credential status without revealing secrets;
- Agent/MCP/skills readiness;
- storage picker/reveal/diagnostic actions;
- native picker/reveal behavior;
- preview/render review without Node, pnpm, Homebrew, or external media tools.

Expected: all pass with the developer package-manager paths absent.

- [ ] **Step 5: Write the final release report**

Include every item required by the approved design: source commit, commands, gate counts, GStreamer/GES versions, runtime inventory counts, recursive dependency results, sanitized acceptance, hashes, signing/notarization/staple/Gatekeeper evidence, and cleanup. The only allowed final status is `passed`; do not translate a red gate into `passed_with_known_debt`.

- [ ] **Step 6: Perform final process and artifact cleanup**

Stop app, browser, Vite, SecurityAgent, runtime probe, and acceptance helper processes owned by the task. Remove proven obsolete generated candidates and temporary isolated homes. Retain the final app, DMG, and evidence report.

- [ ] **Step 7: Run verification-before-completion**

Use `superpowers:verification-before-completion`. Re-run lightweight immutable checks against the retained artifacts and inspect `git status`. If any required gate is red or task-owned state is dirty, return to the owning task instead of claiming completion.

Commit final report changes:

```bash
rtk git add docs output/settings-readiness/final-gate
rtk git restore --staged .superpowers/sdd/progress.md
rtk git commit -m "docs(readiness): record zero-debt packaged release"
```

## Final Acceptance Checklist

- [ ] Source policy passes and frontend source is TypeScript-only.
- [ ] Both TypeScript configurations pass all strengthened strict flags.
- [ ] Tooling scripts pass their separate static/syntax boundary.
- [ ] Complete Rust workspace passes rustfmt and warning-free Cargo check.
- [ ] Canonical all-target Clippy passes with only rare, audited item expectations.
- [ ] Unsupported dotLottie features still fail closed explicitly.
- [ ] Full serial Rust suite passes with native macOS access.
- [ ] Visual capture is deterministic and release comparison is 73/73.
- [ ] Release source and packaged artifacts have no package-manager runtime dependency.
- [ ] GStreamer/GES and every required helper are bundled, signed, dependency-closed, licensed, and inventoried.
- [ ] Signed packaged app passes Settings and rendering acceptance under isolated `HOME` and sanitized `PATH`.
- [ ] Fresh app and DMG are notarized, stapled, Gatekeeper-accepted, hashed, and retained.
- [ ] Final report status is exactly `passed`, with no known-debt exception.
- [ ] No task-owned process, generated candidate, or uncommitted source change remains.
