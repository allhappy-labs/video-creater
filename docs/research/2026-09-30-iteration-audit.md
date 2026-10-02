# Build and test iteration audit

Audit date: 2026-09-30. Scope: local development entrypoints, frontend and Rust verification, cache behavior, test orchestration, and performance evidence. This is a source inspection only; no builds or tests were run, and no wall-clock timings were collected. Performance impacts below are hypotheses until measured.

## Prioritized opportunities

### 1. Measure and shorten the desktop development startup path

**Evidence.** `pnpm dev` delegates to `scripts/tauri-dev.mjs`, which always runs a platform preparation script before starting `tauri dev` (`scripts/tauri-dev.mjs:25-27`). The Linux preparation chain has eight helper/runtime preparation steps, in series (`package.json:12-14`); macOS has nine (`package.json:12-13`). The runtime guide confirms this is the normal desktop development entrypoint (`docs/development/runtime-and-verification.md:3-6`). Individual Rust builder scripts invoke Cargo and then copy outputs, for example the precompose builder (`scripts/build-precompose-sidecar.mjs:34-63`). Cargo avoids recompiling unchanged Rust units, but the app launch still waits for the entire preparation sequence.

**Existing mitigation.** Development helpers use debug profiles and the normal development Cargo target; the native builders have explicit `:dev` variants. This avoids release code generation for most helper builds.

**Recommendation.** Record per-step and end-to-end cold and warm startup durations first. Then skip preparation for outputs whose source/toolchain/config fingerprints still match, and make optional helpers lazy where the app can start safely without them. Preserve a force-prepare path for validating the full bundle. Avoid blindly parallelizing Cargo builders that share a target directory; benchmark any batching or parallel work against CPU, memory, and disk pressure.

**Scoped check.** On one clean checkout and one warm checkout, capture step timings from `pnpm dev` through the first usable editor screen, then repeat after touching one helper source and after touching no source. Acceptance should include unchanged helper hashes on a no-op launch and a correct rebuild when one input changes.

**Confidence.** High that this is a startup gate; medium that fingerprinting or lazy preparation will materially shorten warm starts until timings identify the dominant steps.

### 2. Provide an incremental Rust edit loop alongside the bounded verification lane

**Evidence.** The Cargo dev profile enables incremental compilation (`src-tauri/Cargo.toml:193-195`), while the verification wrapper always points both lanes at `src-tauri/target/verify` and sets `CARGO_INCREMENTAL=0` (`scripts/run-native-verification.mjs:79-89`). The `fast` lane still runs `cargo check --workspace` before exact filtered library tests (`scripts/run-native-verification.mjs:52-60`). The docs distinguish normal Cargo development from verification cache ownership and describe the current fast lane (`docs/development/runtime-and-verification.md:43-45,194-207`).

**Existing mitigation.** Normal unwrapped Cargo commands retain the incremental `src-tauri/target` cache; verification has a separate bounded cache, requires free disk space, and does not automatically remove the development cache (`docs/development/runtime-and-verification.md:196-216`).

**Recommendation.** Keep the bounded, non-incremental verification behavior for reliable release evidence. Add a documented, incremental developer lane that can check the touched crate or root library with a supplied exact test filter in the development target. Make its narrower feature coverage explicit so it cannot be mistaken for release verification. Consider making the fast wrapper's workspace check optional or crate-scoped when the caller knows the affected package.

**Scoped check.** Compare first-run and second-run times for a small Rust edit with the incremental developer lane and current fast lane; also compare a root crate edit and a worker crate edit. Require the focused lane to run the requested test and clearly record its feature/profile scope.

**Confidence.** High that the current fast lane discards incremental compilation and checks the full workspace; medium on expected speedup because target cache contents and actual compile fanout were not measured.

### 3. Avoid running the TypeScript checker twice in the full frontend gate

**Evidence.** `verify:frontend` runs `pnpm lint`, then later `pnpm build` (`package.json:107`). `lint` runs `tsc --noEmit` and checks the Node TypeScript config, while `build` starts with another `tsc` before `vite build` (`package.json:15,100`). The main TypeScript config itself has `noEmit: true` (`tsconfig.json:19`), so the repeated frontend check does not produce build output needed by Vite.

**Existing mitigation.** The commands are separately useful: `build` remains a self-contained checked build, and `lint` also validates `tsconfig.node.json`.

**Recommendation.** Keep standalone `build` safe, but compose the full gate from one TypeScript check plus a bundle-only Vite command. Alternatively, use an explicit gate-only script that avoids the duplicate check. Preserve Node-config checking and ensure local standalone build behavior remains clear.

**Scoped check.** Time `pnpm lint`, standalone `pnpm build`, and the revised frontend gate on the same warm checkout; inspect commands to confirm both TypeScript configs are still checked once and the production bundle is still built.

**Confidence.** High that the frontend `tsc` work is duplicated in the full gate; unmeasured whether it is a material share of total gate time.

### 4. Retain verification timing history and add comparable summaries

**Evidence.** Native verification records elapsed time per step, total time, cache classification, cleanup, disk levels, and target sizes (`scripts/run-native-verification.mjs:108-129,199-236`). It writes the same `native-verification-latest.json` path each run (`scripts/run-native-verification.mjs:17-21,92-96`), and the docs describe that as the output (`docs/development/runtime-and-verification.md:200-207`).

**Existing mitigation.** The report captures enough dimensions to distinguish some warm/cold and disk-cleanup effects.

**Recommendation.** Keep the latest report for convenience and optionally append or rotate timestamped run records with a stable schema. Add a small summarizer for median and p90 duration by lane, cache class, and step. Do not compare runs across different toolchains, feature profiles, or machine classes as if they were equivalent.

**Scoped check.** Run the same fast-lane filter at least five times without source changes, once cold and repeatedly warm; verify history survives subsequent runs and the summary separates cold from warm measurements.

**Confidence.** High that the current path overwrites prior measurements; high that no trend analysis can be done from that one file alone. The potential build improvements remain unmeasured.

### 5. Split broad frontend acceptance into change-oriented gates while preserving the release gate

**Evidence.** The frontend verification script serially runs source policy, tooling policy, a long source-quality test list, lint, unit tests, production build, unused-code scan, all browser specs, and browser visual release QA (`package.json:107`, with the policy test inventory at `package.json:20`). The docs already explain evidence boundaries and say to choose the narrowest lane that proves a change (`docs/development/runtime-and-verification.md:19-45`).

**Existing mitigation.** Individual component commands and narrow Playwright spec invocations are available, and the runtime guide documents them (`docs/development/runtime-and-verification.md:54-76`). Full frontend/release gates remain explicit.

**Recommendation.** Define a few named developer checks around actual change classes: source/type/unit, UI browser spec, visual baseline, and full frontend release. Add path-aware selection only where dependency mapping is dependable; always retain the complete release gate before release. Have the gate report exactly which checks were selected and skipped.

**Scoped check.** Use representative source-only, editor UI, and build-policy changes to confirm the chosen check catches a deliberately introduced local failure in each class, while the full frontend command continues to include every existing gate.

**Confidence.** High that the full gate is broader than needed for every edit and that targeted commands exist; medium that path selection will save meaningful time without missing cross-cutting dependencies.

### 6. Add a project-contained CI gate map and automation entrypoint

**Evidence.** `git ls-files .github .forgejo` returned no tracked workflow files. Verification is instead exposed through package scripts, including `verify:frontend`, `verify:native:fast`, and `verify:release` (`package.json:107-113`). The release gate includes platform-native evidence via the native Rust verification runner (`scripts/run-native-verification.mjs:64-76`).

**Existing mitigation.** The gates are named and runnable locally; the project documents the tiers. This finding does not establish that an external CI system is absent.

**Recommendation.** If CI is not already configured externally, add a small checked-in workflow map: run fast portable checks on ordinary changes, full frontend checks on relevant merges, and native release evidence only on supported macOS runners or release candidates. Pin third-party workflow actions to immutable SHAs per project policy. First verify external CI configuration before duplicating it.

**Scoped check.** Confirm current merge-protection requirements with repository configuration, then ensure each required check maps to a checked-in command and that platform-specific checks are not reported as passed when no matching runner executed.

**Confidence.** High for absence of tracked workflow files in this checkout; low on whether a separate external CI system exists.

### 7. Extend performance coverage from one preview fixture to representative editor workloads

**Evidence.** The current preview playback benchmark uses a fixed six-second, 1920×1080, 60-fps fixture (`scripts/preview-playback-performance.mjs:15-19`) and measures rAF cadence, prepared-frame cadence, load errors, and thresholds (`scripts/preview-playback-performance.mjs:275-335`). Its result is written under the ignored `output/` tree (`scripts/preview-playback-performance.mjs:8-11`).

**Existing mitigation.** It provides a reproducible browser fixture and explicit cadence thresholds. Other visual-release checks and browser flows are also present (`package.json:32-40,107`).

**Recommendation.** Keep the focused preview check, and add representative scale cases for project open, timeline interaction, seeking, and project save/load. Record project dimensions (media count, clips, tracks, effects), startup and interaction latency percentiles, memory, and dropped-frame rate. Start as a local trend report; make thresholds gating only after repeated results establish stable machine-specific baselines. Add at least one native desktop measurement because browser fixtures do not exercise native media and WebView costs.

**Scoped check.** Compare small, medium, and large deterministic projects on the browser fixture and supported desktop runtime, record p50/p95 with environment metadata, and verify output/report correctness before defining regression limits.

**Confidence.** High that current benchmark scope is a single preview playback scenario; medium that the listed editor workloads are current bottlenecks because they have not been profiled in this audit.

## Summary for follow-up planning

The clearest low-risk first steps are to collect startup and gate timings, stop the duplicate TypeScript pass in the full frontend gate, and retain verification history. The most direct Rust edit-loop opportunity is to use the already configured incremental development target for focused checks while preserving the bounded non-incremental lane as release evidence. Larger changes—helper invalidation/lazy preparation, path-aware test selection, package extraction, and new performance thresholds—should follow measured baselines rather than precede them.
