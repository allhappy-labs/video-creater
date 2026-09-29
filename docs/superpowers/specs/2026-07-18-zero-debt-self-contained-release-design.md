# Zero-Debt Self-Contained Release Design

**Date:** 2026-07-18

**Status:** Approved for implementation planning

## Objective

Finish the Settings and runtime-readiness program with repository-wide evidence instead of
scope-limited exceptions. A releasable Video Creater build must be self-contained at runtime,
strictly checked in Rust and TypeScript, visually deterministic, natively tested in the macOS
environment its integration tests require, and shipped without known verification debt.

## Product Invariants

1. The installed application must not require Homebrew, npm, pnpm, Node.js, `node_modules`, or
   another package manager at runtime.
2. A required native library, plugin, model helper, or sidecar must either be bundled and signed
   with its complete dependency closure or be provided by a supported macOS system framework or
   absolute system executable.
3. Build-time package managers may assemble and verify artifacts, but release code must not
   discover required functionality through the user's `PATH`.
4. Application frontend code and frontend tests must be TypeScript (`.ts` or `.tsx`) and compile
   under the project strictness policy.
5. Rust formatting, compiler warnings, and the canonical Clippy policy must be clean. Clippy
   expectations are exceptional, local, justified, and self-expiring.
6. Native integration tests must run with real macOS GPU and media-service access. Sandbox
   restrictions are an execution-boundary concern, not an accepted failing test state.
7. Visual baselines must represent deterministic, reviewed UI output. A stale baseline is a
   failed release gate until it is repaired and re-approved.
8. Final readiness evidence must not use a `known debt`, `host-only failure`, or similar exception
   to convert a red canonical gate into a release pass.

## Current Evidence

The signed application already contains a curated GStreamer and GES 1.28.2 runtime under
`Contents/Resources/render-runtime`. The main executable resolves GStreamer libraries through an
`@rpath` rooted in that directory. The runtime contains the required libraries, 34 plugins, plugin
scanner, native probe, manifest, and license notices.

The remaining readiness debt reproduced on 2026-07-18 is:

- repo-wide `cargo fmt --check` cannot traverse the curated dotLottie source because rustfmt tries
  to resolve unsupported feature modules that are intentionally absent;
- the Rust test build reports dead code and an unused test import;
- the canonical default-feature Clippy audit reports actionable findings plus structural lints;
- the full Rust suite reports three failures inside the restricted execution sandbox, while the
  same Metal, `afconvert`, and bundled GStreamer filmstrip tests pass unchanged with native macOS
  access;
- the fresh visual comparison reports 15 mismatches among 73 scenarios; the Settings pair and all
  34 modern-editor scenarios match;
- application code under `src/` is already 81 `.ts` files and 98 `.tsx` files with no `.js` or
  `.jsx`, and both TypeScript configurations already set `strict: true`;
- build, release, and QA tooling still includes Node `.mjs` files, and several current product or
  diagnostic paths mention `pnpm`, Homebrew installation hints, Temporal CLI tools, or binaries
  resolved through `PATH`.

## Runtime Dependency Architecture

### Classification

Every executable, dynamic library, plugin, helper, and model runtime reachable from a release build
must be assigned exactly one classification:

| Classification | Release rule |
| --- | --- |
| Bundled runtime | Stored inside the app bundle, signed, inventoried, dependency-closed, and launched by an app-relative absolute path. |
| macOS system runtime | Resolved only from a documented absolute system path or Apple framework available on the supported macOS floor. |
| Development-only tool | Excluded from release runtime behavior and release-facing recovery copy; may be used by build, test, or evidence scripts. |

An unclassified runtime dependency fails release verification.

### Bundled components

GStreamer/GES, plugin scanning, compatibility decoding, precomposition, native export, audio
enhancement, semantic encoding, FluidAudio integration, Core ML inspection, and MCP helpers must be
verified using this classification. Bundled components must include licenses and provenance where
their licenses require it.

The release verifier must recursively inspect every packaged Mach-O file, including dynamically
loaded plugins and helper executables. Dependencies may resolve only to:

- another file within the signed app bundle through an approved `@rpath`, `@loader_path`, or
  `@executable_path` relationship; or
- an approved `/System/Library` or `/usr/lib` dependency supplied by macOS.

References to `/opt/homebrew`, `/usr/local`, package-manager prefixes, a developer checkout,
`node_modules`, or unresolved bare executable names fail the gate.

### Runtime commands and recovery paths

Release code must not call `pnpm`, `npm`, `node`, `brew`, `gst-launch-1.0`, `ffmpeg`, `temporal`,
`protoc`, or another externally installed tool by name. Each current occurrence must be handled by
one of the following outcomes:

- replace it with an existing bundled helper or Rust-owned implementation;
- add a reviewed bundled helper and its closed dependency set;
- use an approved macOS system framework or absolute system executable; or
- classify the function as development-only and remove it from the release runtime and release UI.

Release-facing error messages must provide an in-app repair, download, retry, reveal, or diagnostic
action. They must not instruct users to install Homebrew or run a package-manager command.

### Sanitized acceptance environment

Packaged acceptance must launch the signed app with an isolated home directory and a sanitized
`PATH` that contains only the minimum macOS system paths required by the acceptance harness. The
test must exercise Settings health, model operations, GStreamer/GES composition, supported export
delivery, provider credential status, Agent/MCP/skills, storage actions, and native reveal/picker
behavior without inheriting developer package-manager state.

## Rust Quality Policy

### Formatting

The complete workspace must pass one canonical rustfmt command. The curated dotLottie crate must
remain feature-restricted while becoming traversable by rustfmt; unsupported feature modules must
not be restored merely to satisfy tooling.

Formatting output produced before the current rustfmt traversal failure must be reviewed and
committed as one intentional formatting change rather than left as unexplained worktree state.

### Clippy

The canonical Clippy command must cover the workspace, all targets, and the reviewed production
feature set with `-D warnings`. It must not use `--all-features` when that combination deliberately
enables unsupported dotLottie capabilities. A separate policy test must prove that unsupported
feature combinations continue to fail closed.

The implementation must fix:

- dead code and unused imports;
- unnecessary unwraps, borrows, closures, clones, and question marks;
- duplicated or collapsible branches;
- inefficient iterator and collection use;
- derivable implementations and clearer standard-library operations;
- misplaced test modules and other low-risk structural findings;
- large types or argument lists when a focused refactor improves the domain boundary.

Lint suppression is not a cleanup strategy. An intentional structural exception is permitted only
when all of these conditions hold:

1. the structure is required by a versioned protocol, stable external contract, diagnostic payload,
   or clearer domain boundary;
2. changing it would introduce artificial wrapper types, unnecessary allocation, or less reliable
   error evidence;
3. the annotation is attached to the narrowest item;
4. it uses `#[expect(..., reason = "...")]`, not a broad `allow`;
5. the reason names the exact architectural constraint;
6. tests cover the preserved behavior.

Crate-wide and workspace-wide Clippy exceptions are prohibited for this program.

### Native tests

The complete Rust suite must run serially with native macOS access. Metal, system media conversion,
Keychain, notification, and bundled GStreamer integration tests may retain explicit platform guards,
but a supported macOS release machine must execute them rather than silently return early. The final
report must distinguish unit tests from native integration tests while requiring both groups to pass.

## TypeScript Frontend Policy

### Source boundary

All browser/application code and its tests under `src/` must use `.ts` or `.tsx`. A policy check
must fail if `.js` or `.jsx` enters that boundary. JSX belongs only in `.tsx` files.

Node-based build, release, and evidence scripts are tooling rather than shipped frontend code. They
may remain `.mjs` only while they are covered by a separate static-checking boundary. A tooling file
that becomes imported into the application must first migrate to TypeScript.

### Compiler strictness

The application and Vite configuration must retain `strict: true` and add:

- `noUncheckedIndexedAccess`;
- `exactOptionalPropertyTypes`;
- `noImplicitOverride`.

Adoption must fix resulting type errors at their source. Project-wide weakening through `any`,
blanket type assertions, `@ts-ignore`, or disabling strict options is not acceptable. Narrow runtime
validation remains required where untrusted IPC, JSON, filesystem, provider, or agent data enters
the typed frontend.

## Visual Determinism and Baselines

Before changing any of the 15 failing baselines, the visual suite must capture the complete scenario
set twice from the same source commit and compare fresh output against fresh output. Any mismatch in
that comparison is nondeterminism and must be fixed in scenario state, time, animation, font loading,
or asynchronous readiness before baseline approval.

Once deterministic, each changed scenario must be reviewed as baseline/current/diff. Current output
may replace a baseline only when it preserves editor usability, responsive containment, interaction
state, and accessibility expectations. The final release comparison must pass all 73 scenarios at
the configured thresholds.

## Release Verification

The final source commit must produce a fresh app and DMG through the literal release command. The
release report must record:

- source commit and dirty-state policy;
- TypeScript, frontend, rustfmt, Clippy, Rust, Settings E2E, runtime, and visual gate results;
- GStreamer and GES versions;
- bundled runtime file, library, plugin, factory, helper, license, and provenance counts;
- executable and helper RPATH validation;
- complete packaged Mach-O dependency closure;
- sanitized-environment packaged acceptance;
- app and DMG hashes;
- Developer ID signing, hardened runtime, secure timestamps, notarization identifiers, staples,
  Gatekeeper acceptance, and post-staple validation;
- process cleanup and generated-candidate cleanup.

The final status may be `passed` only when every required gate passes. Diagnostic history may remain
available, but it must be clearly marked superseded and must not influence current readiness.

## Testing Strategy

Implementation follows test-first cycles:

1. Add policy regressions for TypeScript-only frontend sources, strict compiler flags, narrow Clippy
   expectations, reviewed Rust features, and forbidden runtime package-manager references.
2. Demonstrate each policy test fails against the current repository state.
3. Make the smallest production or configuration change that satisfies that policy.
4. Run focused tests after each change, then the complete canonical gate for that subsystem.
5. Run the full native, visual, packaged-app, and notarized release sequence from the final commit.

Tests must inspect real release artifacts where possible. Source-text assertions alone are not
sufficient proof for dependency closure, executable resolution, signing, or packaged behavior.

## Worktree and Artifact Hygiene

Task-owned source, formatting, baseline, policy, and documentation changes must be committed using
Conventional Commits. Controller-owned progress state is not staged unless its owner explicitly
authorizes it. Generated runtime candidates are compared with the published runtime; obsolete
task-generated candidates are removed, while unrecognized user-owned files are preserved and
reported.

No application, browser, SecurityAgent, runtime probe, Vite server, or acceptance helper process may
remain after the final audit.

## Non-Goals

- Replacing macOS system frameworks with bundled third-party equivalents.
- Enabling unreviewed dotLottie, codec, or GStreamer features to make `--all-features` succeed.
- Converting every build script to TypeScript when it is not part of the frontend boundary and is
  already covered by the tooling static-checking policy.
- Hiding intentional platform requirements by skipping native integration tests on supported macOS.
- Updating visual baselines without deterministic recapture and visual review.

## Completion Criteria

The program is complete only when:

- the app is demonstrably self-contained at runtime;
- frontend code is TypeScript-only and passes the strengthened strict configuration;
- rustfmt, compiler warnings, and canonical Clippy are clean;
- every lint expectation satisfies the rare-exception policy;
- the complete native Rust suite passes with required macOS access;
- the visual comparison passes 73 of 73 deterministic scenarios;
- Settings and required runtime behavior pass in the packaged signed app under a sanitized
  environment;
- the final app and DMG are signed, notarized, stapled, Gatekeeper-accepted, and retained with their
  evidence;
- the final readiness report has status `passed` and contains no known-debt exception.
