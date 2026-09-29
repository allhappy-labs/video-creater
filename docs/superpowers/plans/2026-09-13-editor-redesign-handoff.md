# Editor Redesign, Gap Closure and Claude Backend — Hand-off

Date: 2026-09-18, recorded on branch `main` at `30b20e0a`. **Corrected on 2026-09-24 at
`cacaa273`**, 15 commits later.

Every section dated 2026-09-18 records the state at `30b20e0a` and is left at that date on purpose.
Seven of the 15 later commits changed something the gates cover — `a7c9b0f5`, `e1418e40`,
`a2e364dd`, `2de9704d`, `23125ee1` (the source-quality gate's own glob, in `package.json`),
`6c3a8da1` and `2cbd1147`; three of them, `a7c9b0f5`, `6c3a8da1` and `2cbd1147`, changed `src/` or
`src-tauri/`. **No recorded full-gate run covered any of the seven** until the
[2026-09-24 project status audit](2026-09-24-project-status-audit.md), and that is how the
`cargo fmt` regression below went unnoticed at three commits' tips — `2cbd1147`, `ab130b42` and
`7553ad1c`. The two source commits since,
`a7e7f943` and `cacaa273`, both came out of that audit and are gated below. Where the audit or the
2026-09-24 re-check contradicts a figure recorded on 2026-09-18, both are given, with the date each
belongs to.

Three workstreams ran back to back on top of the 2026-09-13 editor redesign spec. This note is the
single record of what shipped, what was verified and when, and what is still open. Where it and
[the product backlog](../../product-backlog.md) once disagreed, the backlog rows are now the
authority for status and next action, and this note records the evidence behind them.

Specs and plans:

- [Editor redesign](../specs/2026-09-13-editor-ui-ux-redesign-design.md), plans `2026-09-13-editor-redesign-01…09`
- [Gap closure](../specs/2026-09-16-editor-redesign-gap-closure-design.md), plans `2026-09-16-gap-closure-01…06`
- [Claude agent backend](../specs/2026-09-18-claude-agent-backend-design.md), plans `2026-09-18-claude-agent-backend-01…02`

**297** commits land between `bda87608` (the redesign spec and plans, exclusive) and `30b20e0a`:
142 for the redesign, 118 for the gap closure, 37 for the Claude backend, splitting at each later
workstream's design spec — `4241d203` for the gap closure and `db54bd06` for the Claude backend.
`git rev-list --count bda87608..30b20e0a` is 297, and the history has no merges. An earlier
144 + 117 + 37 = 298 was off by one.

## What shipped

### Editor redesign (plans 01–09, 144 commits)

The commits come from `git log`, matched to each plan by the commit subjects the plans prescribe.
Plans 05 to 09 overlapped, so their commits are interleaved.

| Plan | Final planned commit | Later related fixes |
| --- | --- | --- |
| 01 Foundations extraction | `2c544965` refactor(lib): move remaining pure workspace helpers to lib | none |
| 02 Hard cut and shell | `b3aefbdc` test(editor): cover the desktop and phone editor shell in Playwright | none |
| 03 Timeline | `d0f77aca` test(timeline): cover timeline editing flows at desktop and phone sizes | `43254c24` |
| 04 Preview and properties | `b06620f8` test(editor): cover preview and properties flows at desktop and phone sizes | `a41de382` |
| 05 Left tabs | `0bec43eb` test(editor): cover media, text, captions, effects, and audio panel flows | `0974df28`, `36660c0a`, `a538e3df` |
| 06 AI tab and conversation backend | `678e3313` test(agent): cover auto-apply, review, and failure conversation flows | `1b5c88ae`, `4fee9e8d`, `61610f29`, `2c385b42`, `00902c88` |
| 07 Export, tasks and menus | `0d4e9660` test(editor): cover export, background tasks, and editor menus | `4fd5fbc2`, `e14fa9c1`, `9ee42acd` |
| 08 Transitions | `c63918f8` feat(preview): preview transitions with render parity | `7a9af2fa`, `ea901c9b`, `5d667a85` |
| 09 Mobile, acceptance and docs | `bba57640` docs(editor): document the redesigned editor and update backlog and visuals skill | `4f208d7e`, `b81c1af7`, `3b160364`, `9eba0f28` |

Plan 08's `test(transitions)` commit doesn't exist. `e2e/editor-transitions.spec.ts` arrived in
`3014372f` and `0618b34f`.

### Gap closure (plans 01–06, 117 commits)

| Plan | What landed | Key commits |
| --- | --- | --- |
| 01 Responsive backend and jobs (VC-027, VC-028, VC-021) | A per-project FIFO command queue, project commands off the main thread while a render holds the lease, lease-free render progress, Temporal job reconciliation, and job failure reasons in Background tasks. | `8b730e97`, `42b67c74`, `28271bce`, `bad29823`, `185cdeec`, `6fb36d4f`, `17b71f7a`, `e6c5d35b` |
| 02 Export completeness (VC-019, VC-020) | Export folder, file name, frame-rate override and Master quality reach both the in-process renderer and the Temporal export; exports never overwrite; export artifacts are recorded, Retry restores the settings, and Show in folder reveals a recorded export outside the project. | `0dc10fb6`, `f962f271`, `9823a31c`, `f9e8169b`, `1134679c`, `5402ee62`, `110f3f83`, `7a4c32eb`, `2d272571`, `63c02c41`, `06726c7f` |
| 03 Agent fidelity and Linux frames (VC-023, VC-022, VC-026) | Linux result frames from the canonical frame sampler with graphics overlays; per-turn chat sessions; MCP undo cancels the batch's running generations; compressed capped undo snapshots; `codex/context.rs` split into modules. | `5e3251d3`, `ffd16cbc`, `6beda38d`, `f3989a07`, `0c418dfe`, `dfa9a8c9`, `3162560e`, `f38bd545`, `7442ca81` |
| 04 Media editing (VC-017, VC-018, plus reverse and transition follow-ups) | Validated audio clip speed with pitch-preserving `scaletempo` rendering and preview `playbackRate`; Detach audio as one undo step through timeline, clip tools and the Audio tab; **reverse playback**, which the Task 12 spike decided "go"; sped-up GES clips start at their in-point; Lottie clips keep transition handles. | `0d6a4e0a`, `9bfb8ae2`, `128622b4`, `ba6a8ca5`, `1cd6ded6`, `8a15ece7`, `ee46ffeb`, `563af295`, `2771f2e8`, `8e78dd10`, `f26e27e1`, `5d0c8028`, `a3fa742a`, `b531a1e0`, `58074ae5` |
| 05 NLE interchange (VC-024) | FCPXML dips as Fade To Color; audio-track transitions as crossfades in both formats; upper- and audio-lane FCPXML transitions inside connected storylines; FCPXML captions as connected titles; the writer split into modules; `xmllint` DTD validation wired up. | `82d7bb3b`, `cbb35dfc`, `03636f2d`, `f1f354d3`, `67a1a7a8`, `04f0b4bc`, `05d9e1f4`, `36c510ed`, `766b63b7`, `4dc841f0` |
| 06 Native Linux evidence (VC-025, VC-001) | The Linux desktop smoke split into modules and extended with run context, skipped-step records, retained render/denoise evidence, real X key events for Ctrl+Z and the GTK Edit menu, and AI flows against the real Codex app-server; plus the fixes those runs found. | `e58ad630`, `527bcb81`, `20f75d7c`, `79388a12`, `fc770275`, `2c6ab49a`, `db4fc435`, `7481ae54`, `635e9a37`, `2c66ebd5`, `3d1fffae` |

### Claude agent backend (plans 01–02, 37 commits)

Codex and Claude now sit behind one Rust seam, `AgentTurnTransport` in `src-tauri/src/agent/`.
Everything after the turn — proposal validation, risk classification, deterministic action ids,
atomic apply, snapshot undo — is shared and never sees which agent produced the proposal.

| Area | What landed | Key commits |
| --- | --- | --- |
| Seam and transports | The `AgentTurnTransport` seam with backend resolution; the proposal schema moved out of the Codex transport; the Claude print-mode invocation and its 19-tool read-only surface; the stream-json frame parser; the per-turn Claude CLI process; the shared prompt renderer; Codex routed through the same transport. | `d30e3532`, `e0e1bb42`, `6ebb2e69`, `0cc454e7`, `afde79f6`, `b55c62cf`, `eb755a33` |
| Settings and readiness | An `agentBackend` preference defaulting to Automatic; a zero-token `agent.claude` readiness probe; Advanced → Agent with backend choice, model alias and executable path; plain AI-tab reasons when no agent is available. | `b5dd5db1`, `9f7ddbfc`, `3938c490`, `1ce5c0d5`, `624f97e3` |
| Sessions and routing | Chats mapped onto per-backend agent sessions; each chat's turns on its own Codex thread; `start_codex_conversation_edit_for_project` reads the stored `agentBackend`, `claudeModel` and `claudeExecutablePath` and routes the turn. | `3c9312b8`, `ab7e05a4`, `30b20e0a` |
| Docs and evidence | The backend, its flags and its licence stance documented; the real CLI contract researched; two real turns retained as fixtures. | `19ceddbb`, `db54bd06`, `8e0e4bfb`, `dfdef8af`, `b2f24a18` |

Per spec decision 13 the AI tab gained no backend switch and no model picker, and
`start_codex_conversation_edit_for_project` kept its name and its TypeScript-visible signature.
Nothing Claude-related is bundled: `@anthropic-ai/claude-code` is "all rights reserved", so the
app runs the user's own unmodified binary under the user's own login and never reads, stores or
forwards a credential. See [the agents doc](../../development/agents.md).


## Verification — 2026-09-18 at `30b20e0a`, Linux x64

Ubuntu 24.04 x86_64, at `30b20e0a`, in a clean worktree. Playwright and `verify:frontend` ran under
`flock /tmp/vc-locks/playwright.lock`, because a native-evidence workstream was running at the same
time.

**Every number in this section is the 2026-09-18 run's, at `30b20e0a`.** The whole set of gates was
re-run on 2026-09-24 at `ab130b42` by the
[project status audit](2026-09-24-project-status-audit.md); where its figure differs it is given in
brackets, and [the 2026-09-24 re-run](#the-2026-09-24-re-run) summarises it.

### `rtk pnpm verify:frontend` — passed, exit 0

One run, no re-runs and no flakes.

| Step | Result |
| --- | --- |
| `check:source-quality` | passed |
| `check:tooling-source` | passed, 69 files [**85** on 2026-09-24] |
| `test:source-quality` | 99 passed, 0 failed [**117 passed** on 2026-09-24] |
| `lint` (both `tsc` projects) | passed |
| `test` (Vitest) | 269 files, 2642 tests passed, 48.1 s [269 files, **2,644** tests, 38.46 s] |
| `build` | passed |
| `check:unused` (knip) | passed |
| `test:browser` | 3 Node tests and 72 Playwright tests passed (6.4 min, 1 worker) [same 3 + 72, 5.6 min] |
| `visual:qa:browser-release` | passed: 7 of 7 screenshots, 0 mismatches, `comparisonStatus`, `manifestStatus` and `baselineIntegrityStatus` all `passed`, threshold 0.01 [same, and `platformThresholdStatus` `passed`, platform `linux-x64`] |

The macOS browser visual baseline refresh from redesign plan 02 Task 4 is still pending — no Mac is
available. Only the `linux-x64` baseline was compared.

### Rust tests — passed, no failures

`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, split by `--lib`, `--bins`,
`--doc` and each `--test` target so nothing timed out. Default features already include
`coreml-inspect`, `ges-render`, `gpu-render`, `graphics-render` and `temporal-worker`, so no
`--features` flag was needed. Environment:

- `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`
- `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`
- `VIDEO_CREATER_COMPATIBILITY_DECODER` pointing at the freshly built debug decoder worker
- the precompose sidecar built with `node scripts/build-precompose-sidecar.mjs` (Lottie)
- the MCP sidecar staged with `node scripts/build-macos-release.mjs --prepare-mcp-sidecar --development`

**`settings::agent::tests::prepared_mcp_sidecar_passes_the_schema_v2_probe` passed.** It was the
one environment-only failure of the 2026-09-15 run; staging the sidecar closed it, so this run has
no failures at all and nothing to explain away.

Totals across `--lib`, `--bins`, `--doc` and all 38 `--test` targets: **2,788 passed, 0 failed,
17 ignored.**

The 2026-09-24 re-run of the same split reported **2,807 passed, 0 failed, 17 ignored** (`--lib`
781, `--bins` 193, `--doc` 0, 38 `--test` targets 1,833). **The two totals are not directly
comparable.** `--lib`'s log carries *two* `test result:` lines, because
`project_mutation_lease_blocks_a_separate_process` spawns the test binary again to run
`cross_process_lock_helper` and the child's `1 passed; … filtered out` line lands in the same log.
The 2026-09-24 figure excludes that line deliberately; whether this run's 773 did is not recorded,
so the 19-test difference is somewhere between new tests and that one line.

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| `--lib` | 773 | 0 | 7 |
| `--bins` (19 suites) | 193 | 0 | 0 |
| `--doc` | 0 | 0 | 0 |
| 38 `--test` targets | 1,822 | 0 | 10 |

Each `--test` target:

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| `codex_mcp_server` | 346 | 0 | 0 |
| `project_action` | 203 | 0 | 0 |
| `render_pipeline` | 179 | 0 | 0 |
| `project_split` | 155 | 0 | 0 |
| `project_nle_export` | 111 | 0 | 1 |
| `temporal_workflows` | 101 | 0 | 0 |
| `transcription_models` | 92 | 0 | 0 |
| `project_export` | 87 | 0 | 0 |
| `codex_app_server` | 85 | 0 | 0 |
| `generation_provider` | 64 | 0 | 0 |
| `graphics` | 62 | 0 | 0 |
| `render_transitions_ges` | 49 | 0 | 1 |
| `agent_claude` | 48 | 0 | 2 |
| `one_click_edit` | 43 | 0 | 0 |
| `gpu_graphics` | 41 | 0 | 0 |
| `project_patch` | 29 | 0 | 0 |
| `replicate_generation_provider` | 16 | 0 | 0 |
| `search_index` | 14 | 0 | 0 |
| `temporal_reconcile` | 11 | 0 | 4 |
| `google_generation_provider` | 10 | 0 | 0 |
| `openai_generation_provider` | 10 | 0 | 0 |
| `transcription_e2e` | 10 | 0 | 0 |
| `xai_generation_provider` | 10 | 0 | 0 |
| `media_inspection` | 9 | 0 | 0 |
| `render_template_project` | 9 | 0 | 0 |
| `linux_media_e2e` | 5 | 0 | 0 |
| `reverse_intermediate_spike` | 5 | 0 | 2 |
| `elevenlabs_generation_provider` | 4 | 0 | 0 |
| `export_profiles` | 4 | 0 | 0 |
| `minimax_generation_provider` | 3 | 0 | 0 |
| `audio_sync` | 2 | 0 | 0 |
| `direct_audio_generation_cancellation` | 2 | 0 | 0 |
| `webm_capability` | 2 | 0 | 0 |
| `render_progress_ges` | 1 | 0 | 0 |
| `media_inspection_appkit` | 0 | 0 | 0 |
| `precompose_alpha_ges` | 0 | 0 | 0 |
| `project_export_nested_effect_appkit` | 0 | 0 | 0 |
| `project_export_prores_appkit` | 0 | 0 | 0 |

The last four are `harness = false` and `cfg(target_os = "macos")`: on Linux they link and their
`main` returns without running a test, so they report 0 of everything.

The 17 ignored tests are all deliberate. Sixteen of them said why they were skipped; the
seventeenth, `project::mutation::tests::cross_process_lock_helper`, was a bare `#[ignore]` with no
reason string until `cacaa273` gave it one on 2026-09-24. All 17 are self-declaring now:

- **`--lib`, 7:** `desktop_integration` file-manager (needs a private D-Bus session), the
  cross-process lock helper, the three `provider_credentials` Secret Service tests (each needs a
  particular session-bus state and creates its own keyring collection), and the two production
  model tests (they download ~35 MB and ~670 MB and need the staged speech helper).
- **`agent_claude`, 2:** the two real Claude turns. They spend money and are gated twice, by
  `#[ignore]` and `VIDEO_CREATER_CLAUDE_REAL_TURN=1`.
- **`temporal_reconcile`, 4:** they need a Temporal dev server at `TEMPORAL_ADDRESS`.
- **`reverse_intermediate_spike`, 2** and **`render_transitions_ges`, 1:** 30 s 1080p reverse cost
  measurements.
- **`project_nle_export`, 1:** DTD validation, which needs `VIDEO_CREATER_XMLLINT` and the pinned
  DTDs — `rtk pnpm verify:nle-xml` runs it.

One run-local note, not a product failure: `linux_media_e2e`, `render_pipeline` and
`render_template_project` resolve their helper binaries from a hard-coded
`CARGO_MANIFEST_DIR/target/debug/…` and ignore `CARGO_TARGET_DIR`. This run shared a warm target
directory through `CARGO_TARGET_DIR`, so `linux_media_e2e` first failed all 5 tests with "build the
worker first". Pointing `src-tauri/target` at the shared directory made them resolve, and the
target then passed 5 of 5. The repo's own `REQUIRED_NATIVE_LANES` never runs these three targets
with a redirected target directory, so no shipped lane is affected.

### Formatting and lint

- `cargo fmt --all --check` — clean, exit 0 **at `30b20e0a`, and not continuously since.**
  `2cbd1147` ("feat(agent): make Claude the preferred backend on the user's own subscription") left
  five of its own files unformatted, so the gate went red there and stayed red through `ab130b42`
  and `7553ad1c` until the 2026-09-24 audit ran the gate and caught it. Nothing caught it sooner
  because no full-gate run covered the seven commits after `30b20e0a` listed at the top of this
  note. The audit reported 14 diffs in those five files; every one is rustfmt re-wrapping against
  its default profile (no `rustfmt.toml` exists in the repo), with no behaviour change.
  `a7e7f943` ("style(agent): format the Claude backend sources with rustfmt") reformatted
  `src-tauri/src/agent/turn.rs`, `src-tauri/src/settings/agent.rs` and the three
  `src-tauri/tests/agent_claude/` files, and the gate is clean again from `a7e7f943` onward
  (re-checked at `cacaa273`, exit 0). Because `verify:native:release` runs `cargo fmt --check`
  first, `pnpm verify` / `pnpm verify:release` failed on `main` for that whole stretch.
- `pnpm build` ran inside the frontend gate, so `dist/` existed for the Tauri build script.
- Clippy, with `CXX=c++` and `BINDGEN_EXTRA_CLANG_ARGS="-isystem $(cc -print-file-name=include)"`:
  all three lanes passed with `-D warnings`, exit 0 — `--workspace --all-targets`, then the
  `mcp-server` `video-creater-mcp-server` bin, then the packaged
  `app-runtime,custom-protocol,coreml-inspect,ges-render,gpu-render,graphics-render`
  `video-creater` bin (`pnpm rust:clippy`). The only `warning:` line in the log is the
  `sherpa-onnx-sys` build script saying `SHERPA_ONNX_LIB_DIR` is not set, which is expected without
  the staged Linux speech worker and is not a lint.

### Policy checks

- **`pnpm test:source-quality`** — 99 passed, 0 failed, standalone as well as inside the gate.
  **117 passed, 0 failed on 2026-09-24**, standalone and inside the gate; `23125ee1` widened the
  gate's glob in `package.json` to pick up the export-task smoke helper tests.
- **Legacy paths.** The plan 09 Task 7 `grep` over `src scripts e2e docs/development README.md` for
  `components/workspace|EditorWorkspace|modern-editor|responsive-rail|workspace-layout` finds only
  the deliberate cleanup keys `video-creater.workspace-layout.v1` and
  `video-creater:responsive-rail-state`, in `src/editor/store/legacy-storage-cleanup.ts` and the two
  tests that exercise that cleanup. No legacy component paths remain.
- **Editor source sizes.** No non-test file under `src/editor` exceeds 600 lines (`ok`).
- **knip.** No temporary block remains in `knip.jsonc`.
- **Worktree.** Clean apart from the documentation changes this run made.

### The 2026-09-24 re-run

Every gate above was re-run on 2026-09-24 at `ab130b42`, on the same host, by the
[project status audit](2026-09-24-project-status-audit.md). Read that note for its environment and
its per-target tables; the outcome:

| Gate | Result on 2026-09-24 |
| --- | --- |
| `pnpm verify:frontend` | passed, exit 0 (85 tooling files, 117 source-quality tests, 2,644 Vitest tests, 72 Playwright tests, 7 of 7 screenshots with 0 mismatches on `linux-x64`) |
| Rust suite, same `--lib` / `--bins` / `--doc` / 38 `--test` split | passed, **2,807 passed, 0 failed, 17 ignored** |
| `cargo fmt --all --check` | **failed**, exit 1, 14 diffs in 5 files — fixed by `a7e7f943`, clean again since |
| `pnpm rust:clippy`, all three lanes with `-D warnings` | passed, exit 0 |
| `pnpm test:source-quality` standalone | passed, 117 passed, 0 failed |

Two follow-up commits came out of it, both on 2026-09-24: `a7e7f943` for the formatting, and
`cacaa273` for the one bare `#[ignore]` noted above. `check:tooling-source` (85 files) and
`test:source-quality` (117 passed, 0 failed) were re-confirmed at `a7e7f943`.

## Native evidence and its limits

Every path below under `output/` is git-ignored. Checked on 2026-09-24 in the main worktree:

**Gone, and not re-inspectable without re-running.** `output/gap-closure-01/`,
`output/gap-closure-06/`, `output/gap-closure-07/`, `output/linux-release/` and
`output/nle-xml-validation/` were written inside per-plan worktrees (`vc-gap-06`, `vc-native`) that
have since been removed. `find /home/olhapi -maxdepth 6 -type d -name 'gap-closure-0*'` returns
nothing, and `git worktree list` holds only `main` plus three unrelated `t3code` checkouts. Their
results are recorded here and in the backlog; the artifacts themselves are not. That includes every
folder the gap closure 07 section below cites.

**Still on disk.** `output/linux-desktop-smoke-*` (the 2026-09-15 runs, untouched since), plus
`output/linux-result-frames/` and all nine `output/transition-preview-parity/*/` reports — and those
last two are **not** retained artifacts but test output: `render_transitions_ges::result_frames` and
`render_transitions_ges::parity` rewrite them on every run of that always-on target
(`src-tauri/tests/render_transitions_ges/result_frames.rs:30`, `parity.rs:184`). They were last
written on 2026-09-24 by the audit's Rust run, so their mtimes are that day's, not the day the
evidence was first recorded. Anything under `transition-preview-parity/` or `linux-result-frames/`
can therefore be reproduced by re-running one test target; nothing in the first list can.

**Outside `output/`, some of the gap closure 07 run's inputs and one of its outputs survive under
`/tmp`,** checked on 2026-09-24: the extracted package `/tmp/vc-deb-root-07/` (479 MB, 12 binaries
in `usr/bin` plus `codex-runtime`, `render-runtime`, `sample-project` and `speech-runtime`), the run's
`/tmp/vc-smoke-07/app-support/render-runtime/linux-c8a1d06661198e8a68b3d0762b271d62/`, and the D4
export artifact `/tmp/vc-export-target/Linux Evidence Export.mp4` at **4,939,742 bytes**, the exact
size recorded below. `/tmp/vc-deb-root-06/` and the `.deb` files themselves are gone. These are
`/tmp`, so treat them as about to vanish, not as retained evidence.

### Native packaged-app evidence, 2026-09-18

A separate native-evidence workstream built a `.deb` and drove the packaged app under Xvfb at the
same time as this verification run. Its results are in
[Native Linux evidence — gap closure 07](#native-linux-evidence--gap-closure-07-2026-09-18).
**Nothing in this note claims or summarises them** — read that section for them.

### Linux desktop smoke, 2026-09-15 (debug build)

The debug build ran under Xvfb and `tauri-driver`.
`output/linux-desktop-smoke-render-offmain/` passed project home; Linux platform, render health,
export profiles and capabilities; creating a project; H.264/AAC import; opening the sample; webview
media playback; MP4 H.264 export from the export popover, which reached "Export complete"; settings
health; NLE XML export; Palmier package export; Ctrl+Z undoing a timeline delete in the webview;
the semantic search encoder status (`notInstalled`); and the check that no GPL media libraries are
loaded. The Secret Service step reported "passed" but was skipped, because no `--keyring-root` was
given. Two steps failed: audio denoise (helper not staged) and the agent/MCP self-tests (the debug
build has no Codex or MCP sidecars). `linux-desktop-smoke-render-offmain-denoise/` re-ran denoise
with `VIDEO_CREATER_AUDIO_ENHANCER` set and it passed, but its `evidence.json` showed
`denoiseReported: false`; `3b160364` then made the smoke require a committed denoise cache entry.
`linux-desktop-smoke-nle-undo/` separately passed NLE XML, the Palmier package, Ctrl+Z undo and the
GPL library scan. Everything that run missed was covered on 2026-09-17, below.

### GES transition renders and preview parity

`render_transitions_ges` covers crossfade alpha, dips and wipes including canvas-space masks, range
renders, equal-power audio crossfades and fade-ins, LUT-prepared and flattened clips, clip flips,
and GES main-context isolation. It passes in this run (counts below).

The parity reports under `output/transition-preview-parity/` compare the Rust canonical frame
sampler (`render_canonical_frame_rgba`) against GES output; the threshold is 0.01. They do **not**
compare the in-browser viewer. The `parity` tests write them, so every report below is regenerated
whenever `render_transitions_ges` runs — these figures are from the copies on disk after the
2026-09-24 run, and all nine report `"status": "passed"`:

| Report | Frames | Maximum mismatch ratio |
| --- | ---: | ---: |
| `every-kind` | 13 | 0.00087 |
| `flattened-transitions` | 9 | 0.00087 |
| `speed-transitions` | 10 | 0 |
| `lut-handles` | 6 | 0 |
| `lottie-handles` | 6 | 0 |
| `lottie-handles-speed` | 6 | 0 |
| `reverse-handles` | 6 | 0 |
| `reverse-handles-speed` | 6 | 0 |
| `reverse-spike` | 7 | 0 |

`speed-transitions/` and `lottie-handles/` were introduced in gap closure 04 (`a3fa742a`,
`58074ae5`). An earlier version of this note said they were no longer on disk; they are, because
their tests are always on and rewrite them.

## Native Linux evidence — gap closure 07 (2026-09-18)

A second native-evidence run on current `main`, replacing what the 2026-09-17 run could not finish.
Statuses are copied from each run's `evidence.json`, recorded when the runs happened.

**The folders are gone.** They were written under `output/gap-closure-07/` in the `vc-native`
worktree, which has since been removed; `output/` is git-ignored, so they went with it. Checked on
2026-09-24: no `gap-closure-0*` directory exists on this machine and `git worktree list` no longer
names `vc-native`. Every `output/gap-closure-07/…` path cited below, and the
`output/linux-release/a7c9b0f5ed63/report.json` audit, names where the artifact *was*. The recorded
results stand; the artifacts cannot be re-inspected, and re-inspecting any of them means re-running
the smoke. The exceptions are the three `/tmp` paths listed under
[Native evidence and its limits](#native-evidence-and-its-limits), which do still exist.
Ubuntu 24.04
x86_64, Xvfb `:94`, `tauri-driver` 2.0.6 with `WebKitWebDriver` 2.52.6, `--fake-audio`, one run at a
time (every app instance shares the app preferences and `/tmp/video-creater-editor-project`).

### Build

- **Package:** `src-tauri/target/release/bundle/deb/Video Creater_0.1.0_amd64.deb`, 183,902,986
  bytes, SHA-256 `5d8c3e84845b52db689ebfd8652972b9a6a8b3fb3e485c5dfed7beac994afe1c`.
- **Commit:** `a7c9b0f5`. **Features:** `app-runtime,custom-protocol,ges-render,gpu-render,graphics-render`.
- **Audit:** `passed`, 71 payload files, `missingPayload: []`, `deniedElfDependencies: []`
  (`output/linux-release/a7c9b0f5ed63/report.json`).
- **Render runtime:** freshly built in the worktree — FFmpeg 6.1.6 (LGPL 2.1 or later), gst-libav
  1.24.2, 240 plugins included and 21 excluded, 242 registered, 69 reviewed factories out of a
  750-factory inventory, and `vah264dec`, `vah264enc`, `vah265dec`, `vah265enc` absent (no VA-API on
  this host). The packaged manifest's SHA-256 is
  `c8a1d06661198e8a68b3d0762b271d6240afa1469ff510d2e13314bd906c9b55`, so the materialized runtime is
  `linux-c8a1d06661198e8a68b3d0762b271d62`.
- **Extraction:** `/tmp/vc-deb-root-07` (`dpkg-deb -x`, never installed), 12 binaries in `usr/bin`
  plus `render-runtime`, `codex-runtime`, `sample-project` and `speech-runtime`. Every run set
  `VIDEO_CREATER_APP_SUPPORT_DIR=/tmp/vc-smoke-07/app-support`.
- **An earlier package** was built from `30b20e0a` (183,909,442 bytes, SHA-256
  `571cd49f321efc8ddc9fdaeca2d227ff08cdfe85d51222f99a7cdc70e6b6821e`, audit `passed`) and drove the
  first core run; it is superseded by the one above, which carries the Claude readiness fix.

### D1 — packaged core smoke (`packaged-smoke-final/`, first run `packaged-smoke/`)

`runContext.appKind` is `packaged` with the SHA-256 above. 19 passed, 0 failed, 14 skipped.

| Step | Status | Observed detail |
| --- | --- | --- |
| project home; Linux platform; render health; export profiles; capabilities | passed | `webm`, `mp4H264` and `proResMov` available; `mp4H265` unavailable — "Missing GStreamer factory: vah265enc" |
| create a project; import H.264/AAC media | passed | |
| open the bundled sample; webview media playback | passed | playback advanced to 3.009 s and seeked, over the authenticated `127.0.0.1` media server |
| export the sample to MP4 H.264 from the export popover | passed | `exports/Edison Restoration Demo.mp4`, 4,939,742 bytes; report `succeeded`, 1280×720 h264/aac, 8.023 s; `progressLabels` "Exporting…", "Export complete"; report retained under `export-mp4/` |
| settings reports Linux system health | passed | no macOS-only wording |
| audio denoise through the Linux DeepFilterNet3 helper | passed | `deepfilternet3-onnx-tract-wet-dry-v1`, noise RMS 0.0282 → 0.0118; `pipeline-report.json` and the cache `manifest.json` retained under `denoise/` |
| NLE XML export; Palmier project package export | passed | XMEML 3,104 bytes; package with its manifest |
| Ctrl+Z in the webview undoes a timeline delete | passed | `caption-1` restored |
| provider credentials round-trip through the Secret Service | passed | private GNOME Keyring: `configured: true, source: keychain`, then `configured: false, source: missing` |
| agent and MCP self-tests | passed | `agent.mcpServer`, `agent.proposalValidator` and `agent.codex` all `succeeded`; all four rows ready, including `agent.claude` — "Claude 2.1.270 is signed in as …" |
| semantic search reports its Linux encoder status | passed | encoder `notInstalled` |
| running app processes load no GPL media libraries | passed | inspected `video-creater`, `WebKitNetworkProcess`, `WebKitWebProcess` |
| native menu, export-task, agent flow and Temporal steps | skipped | each names its flag; they ran separately below, except Temporal |

The first core run (`packaged-smoke/`, package `571cd49f`) passed the same 19 steps but reported
`agent.claude` as `failed`. `agent-health/` recorded the diagnosis — `agent.claude.malformedStatus`,
"Claude is installed but its sign-in status could not be read", with `version: 2.1.270` already read
— and that is the product bug fixed in `a7c9b0f5` (see below).

### D2 — AI flows 1 and 2 on the Claude backend (`agent-flows-claude/`)

All four flow steps passed on the first attempt, with no retry. 6 passed, 0 failed, 0 skipped.
`--agent-flows --agent-backend claude --claude-model haiku` stores `agentBackend: "claude"` and
`claudeModel: "haiku"` through `update_app_preferences` before the first prompt, so the turns take
the same stored-preference path Advanced → Agent writes, and hands the choice back to `automatic`
afterwards. The app ran the user's own `/home/olhapi/.local/bin/claude` under their existing login;
nothing was bundled and no credential file was read or written by the run.

- **Readiness:** `agent.claude` `ready` — "Claude 2.1.270 is signed in as oleh@olhapi.com",
  `authMethod: claude.ai`, `subscriptionType: max`.
- **Flow 1** (23.0 s), prompt `Trim one second off the end of the "Restored Edison alternate" clip.
  Change nothing else.` The card is **"Applied to Timeline 1"** with facts `0:08 → 0:07` and
  "Replaces nothing", and the saved timeline changed.
  - **Result frames on Linux: yes.** The card's "Result preview" group held one `img`, complete with
    `naturalWidth` 1920, and never showed "Preview frames aren't available." The PNG is retained at
    `agent-flow-1/frames/renders/agent-result-frame-3-6950-mu701xkh-1/preview-qa/preview-frames/preview-0001.png`
    (920,942 bytes). Screenshot `agent-flow-1-applied.png`.
- **Show changes: yes.** One `clip-highlight` appeared (`agent-flow-1-show-changes.png`).
- **Undo: yes.** The card became "Undone", the saved timeline returned to its pre-edit fingerprint,
  and the highlight count went back to 0 (`agent-flow-1-undone.png`).
- **Flow 2** (13.6 s), prompt `Remove the whole "Audio" track from the timeline, including every clip
  on it.` The card is **"Needs your review"** with planned change "Remove the Audio 1 track". The
  timeline was unchanged while it waited, and unchanged 3 s after **Dismiss**
  (`agent-flow-2-review.png`, `agent-flow-2-dismissed.png`).
- **Spend: $0.0843** for the two turns — `costUsd` 0.068835 and 0.0154177, recorded by the app in the
  project's `context/app-server-conversations.json` and `context/agent-sessions.json`. The per-turn
  budget is `--max-budget-usd 0.5`.
- **Not run, and no longer a blocker:** the same flows through the bundled Codex app-server. The
  2026-09-17 attempt hit that account's model-version error and then its usage limit; this run
  used the Claude backend instead. Since 2026-09-18 Claude is the *preferred* backend, so this is
  optional confirmation of the Codex path rather than missing evidence for the capability.

### D3 — GTK Ctrl+Z and Edit → Undo (`native-menu/`)

Both passed on the first attempt (4 passed, 0 failed).

| Step | Observed detail |
| --- | --- |
| a real `xdotool key ctrl+z` undoes exactly one timeline edit | window `2097155`; two captions deleted, one Ctrl+Z restored `caption-2`, and `caption-1` stayed deleted for 3.17 s |
| Edit → Undo in the GTK menu bar undoes the next edit | method `F10`, keys F10, Right, Down, Return; `caption-1` restored |

X screenshots: `ctrlz-01-before.png`, `ctrlz-02-after.png`, `menu-00-before.png`, `menu-01-f10.png`,
`menu-02-edit.png`.

### D4 — export destination, task progress and editing under a render (`export-tasks-rerun-4/`)

New smoke steps (`--export-tasks`). 5 passed, 0 failed.

| Step | Status | Observed detail |
| --- | --- | --- |
| an export saves to a chosen folder under a chosen name, and Show in folder reveals it | passed, with the reveal unrunnable here | Name typed as `Linux Evidence Export`; the GTK folder chooser opened through `xdg-desktop-portal-gtk` and Ctrl+L set `/tmp/vc-export-target`; the artifact records the absolute path `/tmp/vc-export-target/Linux Evidence Export.mp4`, 4,939,742 bytes. The task row's "Show in folder" button was found and clicked; the app answered "The file manager couldn't show this file." — `no org.freedesktop.FileManager1 service is available; fallback failed: xdg-open could not start: No such file or directory`. This host has neither, so whether a file manager opens is **not observable here**; what is observed is that the button is wired to the export's recorded path and the failure is reported plainly. |
| the Background tasks pill reports a progress fraction while a render runs | passed | pill labels "Exporting…" then **"Exporting · 3%"**, and the popover's task row exposed `role="progressbar"` "Video export progress" with `aria-valuenow` 3 |
| the editor edits, saves and imports while a render runs | passed | webview round trips 44 ms, 3 ms, 2 ms — **no freeze**; importing a 30 s H.264/AAC file during the render succeeded (4 media); the export completed. **But the same timeline delete took 20,359 ms to leave the editor and 20,364 ms to leave the saved project during a 1080p High render, against 772 ms and 779 ms idle** — the delete is not applied optimistically, so it waited on the project write that queued behind the render's lease. The lease half of that is fixed; see [Render leases](#render-leases--a-project-write-no-longer-waits-for-the-encode-2026-09-18). |

Earlier attempts in this group, kept for the record: `export-tasks/` (the folder never changed, and
the responsiveness step's closing undo failed), `export-tasks-rerun-1/` and `-rerun-2/` (the chooser
opened but took no keys), `-rerun-3/` (the chooser step passed once focus was set with
`xdotool windowfocus`). Three smoke defects caused those, all fixed: WebDriver's element `clear`
leaves a React-controlled input's own state behind, a WebDriver screenshot cannot see an X dialog,
and `xdotool windowactivate` needs a window manager, which Xvfb does not run.

### Product bug found and fixed

`fix(agent): read claude auth status as one JSON document` (`a7c9b0f5`). The Claude readiness probe
read the CLI's stdout as JSON-RPC lines, but `claude auth status` pretty-prints one object across
twelve lines, so the first line `{` failed to parse and a working, signed-in Claude 2.1.270 was
reported as `failed / agent.claude.malformedStatus`. Nothing but a real packaged run could see it:
the fixtures printed compact single-line JSON. `ProcessProbeRequest` now carries a `stdout_format`
(`JsonLines`, the default the JSON-RPC probes keep, or `JsonDocument`), the reader is the pure
`parse_probe_stdout`, and `claude --version` keeps the line-oriented path it relies on. Two
pre-existing test races were fixed in the same commit because they made the new coverage flaky: the
executable script fixtures are serialized (a fork in another test thread inherits the writable
descriptor of a fixture being written, and the exec then fails with ETXTBSY), and the
descriptor-leak check reads a settled low-water mark instead of one sample of a process-wide count.
`settings::` is now 171 passed in 5 consecutive runs, where it failed 1 run in 2 before.

### Not run in this workstream

- **Temporal steps.** Release builds leave out the `temporal-worker` feature, so they need the debug
  build; not re-run here. The 2026-09-17 results stand.
- **Codex-backed AI flows**, as above — optional confirmation of the second backend, not a
  blocker.
- **VC-015** packaged transitions on retimed clips, **VC-019** Master and a frame-rate override, and
  **VC-017**/**VC-018** — no steps exist for them yet.

## Render leases — a project write no longer waits for the encode (2026-09-18)

D4 above measured the one responsiveness cost left: the webview never froze, but a timeline delete
took **20,359 ms** to appear during a 1080p High export against **772 ms** idle. Gap closure plan 01
had already moved project-writing commands onto a per-project FIFO queue, so the main thread stayed
free; what the delete still waited for was the lease itself. `render_project_media_after_job_started`
ran inside one `acquire_split_project_mutation_lease` that the render entry points took before the
encode and released after the last result write.

The one lease did two jobs of very different lengths: it serialized canonical project writes, and it
kept two renders (or a render and a preview preparation) from producing the same derived file twice.
It is now two leases, in `src-tauri/src/project/mutation.rs`, and a holder takes them in this order
only — artifacts, then mutation:

| Lease | Lock file | Who holds it, and for how long |
| --- | --- | --- |
| `SplitProjectMutationLease` | `.<project>.mutation.lock` | Canonical project reads and writes, for one read or one write. A render takes it for its start phase (a consistent read, the attempt check, the job-running write) and again for each result write (progress-free: progress is a lease-free snapshot), never across the encode. |
| `SplitProjectArtifactLease` | `.<project>.artifacts.lock` | The project's derived files — `renders/`, precompose intermediates and their caches. Held for a whole render, preview preparation, filmstrip pass or interrupted-render recovery. A canonical write never waits for it. |

What that changed, path by path:

| Path | Before | Now |
| --- | --- | --- |
| `render_media_export_to_split_project_folder` (in-process export, Save Range as Media) | mutation lease from start to finish | mutation lease for the start phase and for each result write; artifact lease for the whole run |
| `render_media_to_split_project_folder`, `render_export_workflow_media_to_split_project_folder` | same | same |
| `render_webm_to_split_project_folder`, `…_for_timeline` (draft render) | same | same |
| `render_prepared_preview_frame_to_split_project_folder[_with_lease]` (canonical capture) | same | same. On Linux the sampler already wrote through `apply_project_actions_to_split_project` only, so only the entry point's hold had to go |
| `extract_rendered_frame_samples_with_lease` | mutation lease across frame decoding | no lease: it decodes the finished output and writes `renders/<jobId>/frames`. Its project read takes the lease for that read alone |
| `prepare_project_preview` (precompose) | mutation lease across the whole preparation, which can run for minutes | mutation lease for the project read; artifact lease for the preparation |
| `cache_timeline_filmstrip_in_split_project_folder` | mutation lease across filmstrip decoding | mutation lease for the project read; artifact lease for the decoding |
| `recover_interrupted_local_render_jobs[_with_lease]` | mutation lease for the whole recovery | unchanged for its canonical writes, plus the artifact lease, because it quarantines render artifacts |
| `run_preview_render_comparison_request` (preview/render QA) | mutation lease across the capture and the comparison | artifact lease across them; its canonical writes take the mutation lease themselves |

Guarantees kept, and why:

- **A user edit cannot corrupt a render.** The render still works from the project it read under the
  lease in its start phase. Everything after — preflight, nested expansion, precompose, the plan,
  graphics, the backend, the probe — reads that snapshot, never the manifest.
- **Result writes are still correct or fail cleanly.** Every one goes through
  `apply_project_actions_to_split_project`, which reloads the canonical project under the lease and
  applies the action to it. A terminal job status that a cancel already wrote is rejected, exactly as
  before.
- **Cancellation still works.** `cancel_render_job_in_split_project_folder` requests cancellation
  out of band and then queues its status write, so it reaches a render whatever lease is held. The
  race the shorter hold opens — the cancel write landing before the render's own `Cancelled` write —
  is benign: the render ignores the result of that write on the cancelled path and still returns
  `render.start.cancelled` / `render.completion`.
- **Renders are still serialized, in process and across processes.** In process by the storage
  mutation lease, which renders still hold for their whole run; across processes by the new artifact
  lease. That matters because precompose publishes a cache entry by renaming a staging directory
  over it, which fails if another writer got there first, and because the app can render while a
  Temporal worker process renders the same project.
- **Package transactions still cannot move the directory under a render.** The only project-directory
  rename is `save_split_project_transactionally`, and in production it is reached only by
  `save_split_project`/`replace_split_project_if_revision` on a project that is *not* yet a split
  package (first save and migration, which `reject_already_split_project` keeps apart from any
  render) and by crash recovery, which runs at open time. A split project's saves take the in-place
  metadata transaction.

### Evidence

Two tests in `src-tauri/tests/project_export/render_lease.rs`, both run against a real GES export:

- `an_encoding_render_holds_no_project_lease` — the test holds the project mutation lease from the
  moment the render records its job as `Running`, and the render's encode, probe and media validation
  still complete (measured at **120 ms** for a 6 s 320×180 draft export) while a `createTrack` write
  sits queued on the real `ProjectCommandQueue`. The render is then parked on its result write and a
  second artifact-lease holder is still blocked, which is the ordering the guarantees above need.
  Releasing the lease completes both.
- `an_edit_during_a_render_does_not_change_its_output` — deleting the only clip through the same
  queue while a 150 s export encodes lands in **484–682 ms** across runs, before the encode
  finishes, and the export still writes **150.02 s** of video. The delete survives the render's
  result writes and the render report is attached.

`project::mutation`'s own unit test `the_artifact_lease_does_not_block_a_project_write` pins the two
boundaries directly, and `project_mutation_lease_blocks_a_separate_process` still pins the
cross-process mutation lock.

**No native re-measurement: the improvement is proven by the tests above and by the code path,
not by a second measurement in the app.** The 20.4 s figure came from the packaged smoke's
`--export-tasks` step, which measures the same delete idle and under a render. Re-running that step
was attempted here against a debug build (`--dev-server --app src-tauri/target/debug/video-creater`,
`tauri-driver` and `WebKitWebDriver` from `~/.cache/vc-desktop-tools/`; the failed run's output was
not retained) and **cannot work on a debug build**, for two reasons worth recording before the next
attempt:

- The step measures the saved half of the delete with its own
  `invoke("load_split_project_from_folder")`, and that command takes the **storage** mutation lease,
  which a render still holds from start to finish. Each probe therefore parks until the export ends.
  Under the packaged release build the 1080p High export finished inside WebDriver's 30 s script
  timeout; under the debug build it does not, and the step fails with `script timed out after
  30000ms`. A first attempt also failed earlier, at `materialize_sample_project_media`, because the
  binary had been built with the tests' `TAURI_CONFIG` override, which empties `bundle.resources`
  and so leaves out `resources/sample-project/`.
- That storage lease is also why only the editor half of the measurement should be expected to
  improve. The delete appears in the editor from the write's own returned project
  (`applyActions` in `src/editor/store/project-slice.ts` sets `result.project`), and that write
  needs the project mutation lease only. The saved half, and the job-status poll in
  `src/editor/store/jobs-slice.ts`, both go through `load_split_project_from_folder` and will keep
  waiting for the render until the storage lease is shortened too. Neither freezes the webview —
  both run off the main thread, and render progress reaches the editor through the lease-free
  `logs/job-progress/` snapshots — but a reader of the next measurement should not expect `savedMs`
  to fall.

So VC-027's next action is that step on a **packaged** app.

### Still holding a long mutation lease, and not changed here

- `caption_visual_frame_cache_in_split_project_folder` holds the mutation lease across fal.ai
  visual captioning, a network round trip per frame. It is not a render path, and the lease is its
  only exclusion between two captioning runs of one media cache, so shortening it needs its own
  cache-ownership decision.
- `write_palmier_project_package` holds it for the whole package export on purpose:
  `render_job_needs_recovery` treats an attempt-less export job as live only because that hold
  exists.
- The Temporal project-bundle activities hold it across `write_project_bundle_package`.

## Native Linux evidence — gap closure 06 (2026-09-17)

Results are copied from each run's `evidence.json`, recorded when the runs happened. The folders
were under `output/gap-closure-06/` in the `vc-gap-06` worktree and no longer exist (see above).
The runs used Ubuntu 24.04 x86_64, Xvfb `:94`, `tauri-driver` with `WebKitWebDriver` 2.52.6, and
`--fake-audio`. They ran one at a time, because every app instance shares the app preferences and
the sample folder `/tmp/video-creater-editor-project`.

### Build

- **Package:** `Video Creater_0.1.0_amd64.deb`, 183,786,488 bytes, SHA-256
  `d05fd34b53425b9e8f36133969f6c6978659a9331c9f0f8787297297a13ba800`.
- **Commit:** `30e44726`. **Features:** `app-runtime,custom-protocol,ges-render,gpu-render,graphics-render`.
- **Audit:** `passed`, with `missingPayload: []` and `deniedElfDependencies: []`.
- **Extraction:** `/tmp/vc-deb-root-06`, with every sidecar (Codex 0.141.0, MCP server, audio
  enhancer, speech, semantic encoder, compatibility decoder, precompose worker) plus
  `codex-runtime`, `render-runtime`, `sample-project` and `speech-runtime`.
- **Render runtime:** manifest includes `scaletempo` and `pngenc`; fingerprint
  `linux-befd0ca764fed6b339234df55c057c6d`.
- **Staleness:** between `30e44726` and the runs, `main` gained only FCPXML caption export
  (`05d9e1f4`), smoke tooling and the jobs merge fix `7481ae54`. None is exercised by the packaged
  steps.

### C1 — packaged smoke

`runContext.appKind` was `packaged`. Passed: project home, Linux platform, render health, export
profiles and capabilities (`webm`, `mp4H264`, `proResMov` available); create a project and import
H.264/AAC media; open the bundled sample and play it in the webview; MP4 H.264 export from the
popover (`exports/Edison Restoration Demo.mp4`, 4,939,593 bytes; the re-run retained a `succeeded`
1280×720 h264/aac report with `progressLabels` "Exporting…" then "Export complete"); Linux system
health in settings with no macOS-only wording; audio denoise through the Linux DeepFilterNet3
helper (`deepfilternet3-onnx-tract-wet-dry-v1`, noise RMS 0.0282 → 0.0118, pipeline report and
manifest retained); NLE XML export (XMEML, 3,104 bytes) and the Palmier package; Ctrl+Z in the
webview restoring `caption-1`; provider credentials round-tripping through a private GNOME Keyring
(`configured: true, source: keychain`, then `configured: false`); the agent and MCP self-tests
(`agent.mcpServer`, `agent.proposalValidator`, `agent.codex` all `succeeded`); the semantic encoder
status (`notInstalled`); and the GPL library scan over `video-creater`, `WebKitNetworkProcess` and
`WebKitWebProcess`. The native-menu, agent-flow and Temporal steps were skipped in this run.

### C2 — GTK Ctrl+Z and Edit → Undo

A real `xdotool key ctrl+z` undid exactly one timeline edit: of two deleted captions, `caption-2`
was restored and `caption-1` stayed deleted for 3 s. Edit → Undo in the GTK menu bar failed the
first time (no item was highlighted after Home, so Return did nothing) and passed after `db4fc435`
changed the navigation to F10, Right, Down, Return. The GPL library scan passed in both runs.

### C3 — AI flows through the bundled Codex app-server: **failed**

Agent flows 1 and 2 both failed with an app-server 400: "The 'gpt-5.6-sol' model requires a newer
version of Codex". The pinned `@openai/codex` 0.141.0 reads the user's `~/.codex/config.toml`,
which selects `gpt-5.6-sol`, and its `model/list` offers only `gpt-5.5`. The retry — with a
test-only wrapper that ran the same bundled binary with `-c model="gpt-5.5"`, removed afterwards,
leaving the user's configuration and credentials untouched — hit the account's usage limit
(`usageLimitExceeded`). No result frames were produced on this path. Linux result frames in the AI
card were observed at the native boundary the next day through the Claude backend (D2), so this
is a gap in the Codex path only.

`635e9a37` since makes a turn check the thread's model against `model/list` and use the server's
default when the configured model isn't listed, retrying once after the version error when listing
fails. Evidence for that fix is fake-server tests (`codex_conversation` and `codex_app_server`
`model_fallback`) plus a no-turn probe of the real 0.141.0 sidecar
(`codex::model_selection::tests::pinned_codex_model_check_runs_without_a_turn`): thread model
`gpt-5.6-sol`, listed `gpt-5.5` and `codex-auto-review`, turn model `gpt-5.5`.

### C4 — Temporal steps, debug build

`runContext.appKind` was `debug`, because release builds leave out `temporal-worker`. Passed:
render health, create a project, import H.264/AAC; installing the transcription model from settings
(already installed: `nvidia/parakeet-tdt-0.6b-v3`, sherpa_onnx, 670,478,772 bytes); selecting
Temporal execution in advanced settings; transcribing imported speech through the Temporal worker
(43 words); analyzing speech in the desktop process (8 speech ranges); and the GPL library scan.
The MP4 export through the Temporal worker failed twice — the job completed with a run id and the
file was saved, but the pill stayed "Exporting…" — and passed after `7481ae54`
(`exports/Edison Restoration Demo (5).mp4`, 4,939,742 bytes, job `completed`, run id
`01a0af55-a1c2-7094-afe9-18f1acdd3d49`, render reports retained).

### Rust evidence

The MCP sidecar probe and the three Secret Service provider tests each passed in their own process
against their own keyring. Running both Secret Service provider tests in one process failed on test
order, not on the product: the locked test locked the shared keyring's default collection first.
`2c66ebd5` gives each test its own GNOME Keyring collection, restores the previous `default` alias
and holds a lock so they cannot interleave; both then passed in one process at `--test-threads=1`
and `--test-threads=4`, with the default collection still unlocked afterwards.

## Documented spec deviations that still hold

- **AVFoundation and transitions.** The AVFoundation exporter rejects render plans that contain
  video or audio transitions (`avfoundation_backend::reject_transitions`), so those renders fall
  back to GES on macOS (VC-016).
- **ProRes.** It is shown disabled with its reason, rather than hidden.
- **The gear button.** Its accessible name is "Editor menu", and "More" on phones under 380 px.
- **Approved AI generations start automatically.** Editor Undo and MCP Undo cancel the runs in the
  app process and at fal.ai and Replicate, but not Temporal runs (`00902c88`, `0c418dfe`).
- **NLE interchange (VC-024).** FCPXML dip to white exports as a cross dissolve; wipes export as
  cuts in both formats; an FCPXML transition whose connected storyline would anchor on a retimed
  primary clip exports as a cut, as does a caption over one (it stays a flat connected title in the
  spine); transitions touching a **reversed** clip export as cuts, because neither format writes
  reverse (`fcbf4f65`); and XMEML still writes all video tracks into one `<track>` and all audio
  into one `<track>`.
- **Agent result frames off macOS.** macOS captures a one-frame native render; every other platform
  uses the Rust canonical frame sampler (`79b1dba5` gates the sampler module off macOS). Where a
  frame cannot be captured the AI card still shows its fallback.

### Deviations from the 2026-09-15 list that are now closed

Removed from the list above, with what closed them:

- "Audio clips have no Speed tab" — the Audio Properties Speed tab shipped in `128622b4`, on a
  validated `updateAudioClipSpeed` action (`9bfb8ae2`) with pitch-preserving `scaletempo`
  rendering (`ba6a8ca5`) and preview `playbackRate` (`1cd6ded6`).
- "Reverse isn't available: the project model has no reverse playback" — the gap closure 04 Task 12
  spike decided **go** ([research](../../research/2026-09-16-reverse-intermediate-spike.md),
  `2771f2e8`); reverse landed in `8e78dd10`, `f26e27e1` and `5d0c8028`.
- "Detach audio isn't available" — the `detachAudio` action (`8a15ece7`), the timeline, clip-tools
  and Audio-tab entry points (`ee46ffeb`) and its render and NLE export (`563af295`).
- "The export popover's Save to, file name and frame-rate fields have no backend inputs, and Master
  quality is disabled" — all four reach the backend since gap closure 02 (`2d272571`, `7a4c32eb`,
  `b2b619ec`, `110f3f83`, `9823a31c`).
- "Agent result frame capture needs AVFoundation and likely fails on Linux" — Linux captures frames
  with the canonical frame sampler and draws graphics and generated video into them (`5e3251d3`,
  `ffd16cbc`).
- "FCPXML exports dips as cross dissolves" — dip to **black** is now a Fade To Color transition with
  a verified effect uid (`cbb35dfc`). Dip to white and wipes are still in the list above.

## What is still open

### Native evidence not yet recorded (Linux)

The 2026-09-18 native-evidence run closed several of these; see
[Native Linux evidence — gap closure 07](#native-linux-evidence--gap-closure-07-2026-09-18) for what
it observed. What is still missing:

| Row | Missing native evidence |
| --- | --- |
| VC-001 | Nothing blocking. Flows 1 and 2 passed on the Claude backend on 2026-09-18 — applied result frames, Show changes and Undo, then a review edit and Dismiss. The Codex path is unrecorded and optional: Claude is the preferred backend, so no capability waits on that account. |
| VC-015 | Packaged transitions on retimed clips. No smoke step exists for them. |
| VC-017 | Native WebKitGTK and packaged evidence that preview audio keeps pitch at non-1× speed. |
| VC-018 | Packaged Detach audio and its single Undo. |
| VC-019 | **Master** and a frame-rate override. The chosen folder and chosen name half passed on 2026-09-18 at 720p High. |
| VC-020 | Whether "Show in folder" opens a file manager. On 2026-09-18 the button was found on the task row of an export written outside the project folder and clicked, and the app reported "The file manager couldn't show this file." — this host has no `org.freedesktop.FileManager1` service and no `xdg-open`, so the outcome is **not observable here**; it needs a host with a desktop file manager. |
| VC-022 | The **Codex** app-server half, which is optional confirmation of the second backend. An MCP-free agent Undo restored the project on Linux on 2026-09-18 through the Claude backend. |
| VC-027 | A packaged re-measurement of the delete latency. The webview freeze was measured and is fine (round trips 44/3/2 ms, an import succeeded during the render). The **20.4 s delete** the run exposed is fixed in the backend — a render now takes the project mutation lease for its start phase and its result writes only ([Render leases](#render-leases--a-project-write-no-longer-waits-for-the-encode-2026-09-18)) — but that is proven by tests, not by a second run of the smoke's `--export-tasks` step. |
| VC-029 | Whether a Tauri-spawned child inherits the macOS keychain login (no Mac on this host). The packaged Claude turn through the app's own command path, its preference read and its history write were observed on 2026-09-18. The `ANTHROPIC_API_KEY` auth mode is no longer unrecorded: all four auth states and both child-environment behaviours are covered by stub executables (`src-tauri/tests/agent_claude/auth.rs`), and a `claude.ai` session withholds the variable so a subscription cannot be silently shadowed. |

### macOS-only, and not runnable on this host

- **Browser visual baselines.** The macOS refresh from redesign plan 02 Task 4 is still pending. Only
  the `linux-x64` baseline was compared, in every run so far including this one.
- **⌘Z single undo (VC-025).** The manual macOS check. The Linux equivalents — a real X `ctrl+z` and
  GTK Edit → Undo — passed on 2026-09-17.
- **AVFoundation transitions (VC-016).** Composing transitions in the AVFoundation exporter instead
  of falling back to GES.
- **Retimed audio on macOS (VC-017).** AVFoundation retimed audio falls back to GES, and the macOS
  GStreamer runtime's `scaletempo` requirement (`d59b4e11`) has never been checked on a Mac.
- **Master on macOS (VC-019).** Master equals Final under AVFoundation; unverified.
- **Claude and the macOS keychain (VC-029).** Whether a Tauri-spawned child inherits the macOS
  keychain login.
- **Notarization (VC-008, `blocked`).** Developer ID notarization and stapling, and clean-Mac
  packaged evidence.
- **Four test targets.** `project_export_prores_appkit`, `project_export_nested_effect_appkit`,
  `media_inspection_appkit` and `precompose_alpha_ges` are `harness = false` and `cfg(target_os =
  "macos")`. On Linux they build and their `main` returns without running anything, so they
  contribute 0 tests to every count in this note.

### Other open work

- **NLE round-trip (VC-024).** No public reference export verifies FCPXML dip to white, wipe effect
  ids, or a connected storyline anchored on a retimed primary clip, so each exports as a cut or a
  cross dissolve ([references](../../research/2026-09-16-nle-transition-interchange-references.md)).
  Opening the exported XML in Premiere Pro and DaVinci Resolve cannot be done on Linux. Resolve's
  own Dip To Color Dissolve defaults to white, so a real import should confirm that a dip to black
  with no colour parameter imports as black.
- **H.265 Master (VC-019).** Unverified: this host has no VA-API encoder.
- **Codex sidecar pin (optional).** `@openai/codex` is pinned at 0.141.0. 0.154.0 lists
  `gpt-5.6-sol` and changed the app-server methods the app uses only additively, but it adds a
  `codex-code-mode-host` helper and has not been checked with a real turn. Bumping it is a
  nice-to-have: Codex is a supported alternative, not the default, so nothing waits on it.
- **Product direction.** VC-007 moved to `implemented` on 2026-09-24: all six pieces of its old next
  action ship, and what remains is an AI card's Undo staying enabled until it is pressed and the
  impact carrying no affected track ids. VC-004 and VC-011 are in `discovery`; VC-005,
  VC-006, VC-009, VC-012, VC-013, VC-014 and VC-016 are `idea`s; VC-002, VC-003 and VC-010 are
  `implemented` with productization next actions. See the backlog for each.
