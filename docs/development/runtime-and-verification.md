# Runtime and Verification Development

## Runtime commands

Use `rtk pnpm dev` for normal development. It prepares native helpers, starts the Tauri
desktop runtime, and starts the internal Vite server through Tauri.

`rtk pnpm dev:web-runtime` starts only Vite. A regular browser is deliberately disconnected
and renders the desktop-host connection shell. It does not receive a fake backend and cannot
read or mutate projects. Browser connection, discovery, pairing, authentication, scoped media,
editing, agent proposals, rendering, and downloads are implemented by the headless host described
in the [remote web host design](../superpowers/specs/2026-09-24-remote-web-host-design.md). Live
cross-device readiness still depends on a verified Tailscale Serve route on the host machine.

`rtk pnpm visual:qa:browser` is a separate development-only fixture runtime. Its explicit
fixture marker supplies deterministic operations and events for browser UI verification.
Fixture evidence does not substitute for desktop or packaged-runtime evidence.

## Evidence boundaries

Browser fixture flows are the acceptance gate for the editor redesign
([spec](../superpowers/specs/2026-09-13-editor-ui-ux-redesign-design.md)): the eight acceptance
flows must pass through the UI at 1440×900 and at the iPhone 17 Pro viewport (402×874) without
console errors. They prove web UI behavior over deterministic fixture handlers only. Native Tauri
checks recorded during the redesign, such as GES transition renders and the Linux desktop smoke
run, are recorded evidence, not release proof. Native desktop evidence for the redesigned editor
is still partial; see the [product backlog](../product-backlog.md) and
[Linux desktop](linux.md) for what remains.

## Verification tiers

Choose the narrowest lane that proves the change:

```bash
rtk pnpm verify:frontend
rtk pnpm verify:native:fast -- frame_compositor::gpu::tests::gpu_matches_canonical_cpu_blends_with_documented_tolerance
rtk pnpm verify
```

`verify:frontend` runs source policy, TypeScript, frontend tests, the production frontend
build, and fixture-backed browser baselines. It does not invoke Cargo or build native helpers.

`verify:native:fast` runs Rust formatting, a default workspace check, and each supplied exact
library test. It excludes release-only feature combinations and native AppKit lanes.

`verify` is the complete release gate. It retains all three shipped Rust profiles:

1. default workspace across all targets;
2. MCP-only sidecar;
3. packaged custom-protocol desktop binary with production render features.

It also runs the full native suite and the required platform-native lanes.

## Editor browser flows

`rtk pnpm test:browser` runs the Playwright CLI policy test, then every `e2e/*.spec.ts` against a
Vite server on port 4178. Failure traces land in `output/playwright/app-smoke/`. Run one spec with
`rtk pnpm exec playwright test e2e/editor-acceptance.spec.ts`; install Chromium once with
`rtk pnpm playwright:install`.

| Spec | Covers |
| --- | --- |
| `app-smoke.spec.ts` | Home, Settings, and the editor shell at desktop and narrow widths without browser errors. |
| `editor-shell.spec.ts` | The fixed desktop layout and tab switching; the iPhone 17 Pro bottom tool bar and sheets. |
| `editor-timeline.spec.ts` | Select, split, undo, drag, Set duration, keyboard split, track rows above the overview bar, and phone tap, split, and scroll. |
| `editor-preview-properties.spec.ts` | Properties docking and overlay, one undo step per commit, keyframes, inline caption editing, and the phone Adjust sheet. |
| `editor-panels.spec.ts` | Media, Text, Captions, Effects, Audio pause review, and Generate cost, plus the phone Media sheet. |
| `editor-ai.spec.ts` | AI tab auto-apply, review, auto-apply off, and failure at desktop and phone; missing agent, Organize with AI, and Ask AI. |
| `editor-export.spec.ts` | The Export button and ⌘E open the Export popover. |
| `editor-export-tasks.spec.ts` | An MP4 1080p export followed through Background tasks, Connect external agents copy, and the shortcuts sheet per platform. |
| `editor-tasks.spec.ts` | The Background tasks popover and render details; the phone tasks sheet with worker preflight. |
| `editor-menus.spec.ts` | The gear menu opens the shortcuts sheet and agent dialogs at desktop and phone. |
| `editor-transitions.spec.ts` | A crossfade from Effects with its duration set in Properties and undone, each transition kind previewed at its midpoint, and the phone `+` flow. |
| `editor-mobile.spec.ts` | The iPhone 17 Pro top bar and swipe-to-close sheets, touch long press, the 375 px More menu, and iPad 820×1180. |
| `editor-keyboard.spec.ts` | Acceptance flow 5 (edit a clip) completed keyboard-only on desktop. |
| `editor-acceptance.spec.ts` | The eight spec acceptance flows at 1440×900 and 402×874. Each end state is captured to `output/editor-acceptance/`. |

### Fixture runtime marker

Specs open the editor through `e2e/support/editor-fixture.ts`, which sets
`window.__EDITOR_FIXTURE_RUNTIME__` (typed as `EditorFixtureRuntimeMarker` in
`src/lib/runtime/bootstrap.ts`). The marker is honored only in DEV builds, and
`src/vite-build-policy.test.ts` asserts that production bundles contain no fixture modules.

| Flag | Implies |
| --- | --- |
| `enabled` | Required; selects the fixture transport instead of Tauri. |
| `panelFixtures` | Editor panel handlers: an effect catalog and detected silences on the sample. |
| `conversationFixture` | AI turns answered by keyword ("tighten" a safe cut, "generate" a review bundle, "fail" a validation failure, anything else a safe caption fix), with apply, undo, result frames, and chats against the folder-backed sample. |
| `tasksFixture` | Background tasks seeded into the sample, plus the task details commands. |
| `exportFixture` | The export capability report, a render that completes over three folder reloads, XML and package exports, and the task handlers. Tasks are seeded only with `tasksFixture`. |
| `acceptanceFixture` | Every handler the acceptance flows reach over a sample that starts untranscribed. Implies `panelFixtures`, `conversationFixture`, and `exportFixture`, and adds fixture media import, preview, filmstrips, backgrounds, search, transcription with speakers, and media generation. |
| `platform` | Host platform reported by the fixture backend (`macos` by default, or `linux`). |
| `settingsFixtureId`, `preferences`, `exportCapabilities` | Settings fixture state, app preferences, and export capabilities returned by the fixture backend. |

Accepted "Show in folder" reveals are recorded on `window.__EDITOR_FIXTURE_REVEALS__` for assertions.

### Editor policy tests

`test:source-quality` includes three editor policy tests:

- `scripts/editor-source-policy.test.ts`: editor files stay under 600 lines; editor code and UI
  primitives use theme tokens instead of raw hex or `white/`/`black/` opacity colors; native APIs
  are reached only through runtime adapters; no `window.prompt` or `window.confirm`; and
  user-visible copy avoids internal engine names.
- `scripts/knip-temporary-block.test.ts`: `knip.jsonc` has no temporary editor-redesign ignore
  block and no `ignoreIssues`.
- `scripts/linux-desktop-smoke-selectors.test.ts`: every accessible name and `data-testid` the
  Linux desktop smoke looks up, across every `scripts/linux-desktop-smoke*.mjs` module, still exists
  in `src/`; the smoke no longer drives the pre-redesign editor; and each smoke module stays under
  600 lines.

The smoke's pure helpers have their own unit tests in `test:source-quality`:
`linux-desktop-smoke-steps.test.ts` (options, step statuses, run context),
`linux-desktop-smoke-retain.test.ts` (retained evidence and export artifacts),
`linux-desktop-smoke-tools.test.ts` (extracted tool environments and X screenshots) and
`linux-desktop-smoke-agent.test.ts` (timeline fingerprints, AI flow prompts and result frames).

## GES render tests

The `ges-render` feature is on by default, so the GES-gated integration tests, such as
`render_transitions_ges`, build with a plain `cargo test`. They need a resolvable render runtime.
Linux never falls back to the system GStreamer plugin directory, which also holds GPL plugins, so
stage the license-filtered runtime and point the tests at it:

```bash
rtk pnpm build:linux-media-runtime --output <runtime-dir>
VIDEO_CREATER_RENDER_RUNTIME_ROOT=<runtime-dir> \
  rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_transitions_ges -- --test-threads=1
```

Without `--output` the script stages `src-tauri/resources/render-runtime`, which debug builds find
without the variable. `project_export` renders that extract review frames also start the
compatibility decoder. Set `VIDEO_CREATER_COMPATIBILITY_DECODER` to a built worker, for example
`src-tauri/target/debug/video-creater-compatibility-decoder` from
`cargo build --manifest-path src-tauri/Cargo.toml -p video-creater-compatibility-decoder`, or stage
the sidecar with `rtk pnpm build:compatibility-decoder:linux:dev`, which debug builds find in
`src-tauri/binaries/`. Linux-specific runtime and smoke checks are in [Linux desktop](linux.md).

## Temporal reconciliation against a dev server

`tests/temporal_reconcile.rs` checks how unfinished Temporal jobs are reconciled with their
workflows. Its fake-describer tests and the closed-port tests run with a plain `cargo test`. The
`dev_server_*` tests are ignored because they need a Temporal dev server. They cover a running
workflow with no worker, a terminated workflow, a workflow that never started, and a namespace the
server doesn't have (jobs stay untouched and the result names the missing namespace).

Start a dev server without a system install. The official image is test-only tooling and is never
bundled:

```bash
rtk docker run --rm -d --name vc-temporal-dev -p 7233:7233 -p 8233:8233 temporalio/temporal:latest server start-dev --ip 0.0.0.0
rtk docker logs vc-temporal-dev   # wait for "Temporal Server:"
```

If the image isn't available, the MIT `temporal` CLI works too. Unpack it under `/tmp` and run
`/tmp/vc-temporal-cli/temporal server start-dev --headless`. Check the flags with
`temporal server start-dev --help` first.

Run the ignored tests, then stop the server:

```bash
TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' TEMPORAL_ADDRESS=http://localhost:7233 \
  rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_reconcile -- --ignored --test-threads=1 --nocapture
rtk docker stop vc-temporal-dev
```

All four `dev_server_*` tests must be listed as `ok`. Save the output under `output/`, which is
git-ignored, as evidence.

## NLE XML validation

`rtk pnpm verify:nle-xml --output <dir>` validates exported Premiere XMEML and DaVinci FCPXML
against their DTDs:

1. It uses `xmllint` from `PATH`, or extracts it from the Ubuntu `libxml2-utils` `.deb` (MIT) into
   `/tmp/vc-smoke-tools/libxml2-utils` (`--tools-dir` changes the folder).
2. It downloads the FCPXML 1.10 DTD and extracts the XMEML v5 DTD from Apple's FCP XML reference
   into `/tmp/vc-smoke-tools/nle-dtd`, and checks both against the SHA-256 pins in
   `scripts/nle-xml-validation-sources.mjs`. The DTDs are Apple-copyrighted: never commit them.
3. It runs the ignored `validation::nle_exports_validate_against_the_dtds` test in
   `project_nle_export` with `VIDEO_CREATER_XMLLINT`, `VIDEO_CREATER_NLE_DTD_DIR` and
   `VIDEO_CREATER_NLE_VALIDATION_OUTPUT` set. The test writes a corpus of exports to
   `<dir>/corpus`, runs `xmllint --dtdvalid` on each file, and records `results.json`.
4. It writes `<dir>/report.json` (xmllint version, DTD hashes, cargo exit code, pass and failure
   counts). It exits non-zero, and says "not run", when tools or DTDs can't be fetched.

The XMEML v5 DTD is permissive (`(a | b)*` content models), so the always-on
`validation::nle_exports_have_consistent_structure` test carries most of the XMEML evidence: it
checks transition placement, `-1` clip edges, links, source ranges and FCPXML storyline anchoring
with `roxmltree`. Effect ids the writers emit cite
`docs/research/2026-09-16-nle-transition-interchange-references.md`.

## Cargo cache ownership

Normal Cargo development continues to use `src-tauri/target` with incremental compilation.
Complete and focused native verification use only `src-tauri/target/verify` with
`CARGO_INCREMENTAL=0`.

The verification wrapper:

- requires at least 20 GiB free before Cargo starts;
- retains a verification cache up to 24 GiB;
- validates the exact verification target before cleanup;
- rejects symlinks, sibling targets, traversal, variables, and broad paths;
- remeasures and enforces the limit after success, failure, or cancellation;
- writes `output/build-metrics/native-verification-latest.json`.

Automatic verification cleanup never selects the development target. If the development cache
must be removed, invoke the explicit command:

```bash
rtk pnpm clean:cargo:dev
```

It prints the exact validated target and measured size before running `cargo clean`.
