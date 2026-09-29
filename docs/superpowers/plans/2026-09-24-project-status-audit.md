# Project Status Audit — 2026-09-24

Date: 2026-09-24. Branch: `main`, at `ab130b42`, worktree clean at the start and end of the run.
Host: Ubuntu 24.04 x86_64 (Linux 6.8.0-139-generic), 1 machine, no Mac available.

This is an independent audit, not a summary of the
[hand-off](2026-09-13-editor-redesign-handoff.md). Every gate below was re-run here; every
documentation claim called out below was checked against the code, the tests or the filesystem.

## 1. Gate results from this run

### Summary

| Gate | Result |
| --- | --- |
| `pnpm verify:frontend` (under `flock /tmp/vc-locks/playwright.lock`) | **passed**, exit 0 |
| Rust suite, split by `--lib`, `--bins`, `--doc` and all 38 `--test` targets | **passed**, 2,807 passed, 0 failed, 17 ignored |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | **FAILED**, exit 1, 14 diffs in 5 files |
| `pnpm rust:clippy` (all three lanes, `-D warnings`) | **passed**, exit 0 |
| `pnpm test:source-quality` (standalone) | **passed**, 117 passed, 0 failed |

`pnpm verify` / `pnpm verify:release` therefore **fails on today's `main`**, because
`verify:native:release` runs `cargo fmt --check` as its first step
(`scripts/run-native-verification.mjs`, `buildVerificationPlan`).

### `rtk pnpm verify:frontend` — passed, exit 0

One run, no re-runs, no flakes, no retries.

| Step | Result |
| --- | --- |
| `check:source-quality` | passed |
| `check:tooling-source` | passed, **85 files** (69 on 2026-09-18) |
| `test:source-quality` | **117 passed, 0 failed** (99 on 2026-09-18) |
| `lint` (both `tsc` projects) | passed |
| `test` (Vitest) | **269 files, 2,644 tests passed**, 38.46 s |
| `build` | passed |
| `check:unused` (knip) | passed |
| `test:browser` | 3 Node tests + **72 Playwright tests passed**, 5.6 min, 1 worker |
| `visual:qa:browser-release` | passed: 7 of 7 screenshots, **0 mismatches**, `comparisonStatus`, `manifestStatus`, `baselineIntegrityStatus` and `platformThresholdStatus` all `passed`, threshold 0.01, platform `linux-x64` |

Only the `linux-x64` baseline was compared. The macOS baseline refresh is still pending — see
the open-work list.

### Rust tests — 2,807 passed, 0 failed, 17 ignored

Every target exited 0. Default features (`app-runtime`, `coreml-inspect`, `ges-render`,
`gpu-render`, `graphics-render`, `temporal-worker`) were used, so no `--features` flag was needed.

Environment (all of it needed on this host, all of it environmental, none of it product):

- `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`
- `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`
- `VIDEO_CREATER_COMPATIBILITY_DECODER` and `src-tauri/target/debug/video-creater-compatibility-decoder`, rebuilt this run
- the precompose sidecar rebuilt with `node scripts/build-precompose-sidecar.mjs` (Lottie)
- the MCP sidecar staged with `node scripts/build-macos-release.mjs --prepare-mcp-sidecar --development`
- `CXX=c++`, `BINDGEN_EXTRA_CLANG_ARGS="-isystem $(cc -print-file-name=include)"`, `GST_GL_WINDOW=dummy`

**The staged Linux render runtime did not need rebuilding.** `/tmp` was cleared since 2026-09-18
(`/tmp/vc-deb-root` is gone), but the `linux-3451661e…` runtime's 247 symlinks point at
`~/.cache/video-creater-linux-build/`, not at `/tmp`, and **0 of them are broken**. Eight
`compatibility-registry-*.bin` and five `registry-*.bin` fingerprints remain under
`render-runtime/`; only the one materialized `linux-3451661e…` directory exists.

| Target group | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| `--lib` (1 binary) | 781 | 0 | 7 |
| `--bins` (19 suites) | 193 | 0 | 0 |
| `--doc` | 0 | 0 | 0 |
| 38 `--test` targets | 1,833 | 0 | 10 |
| **Total** | **2,807** | **0** | **17** |

Per `--test` target:

| Target | Passed | Ignored | | Target | Passed | Ignored |
| --- | ---: | ---: | --- | --- | ---: | ---: |
| `codex_mcp_server` | 346 | 0 | | `transcription_e2e` | 10 | 0 |
| `project_action` | 203 | 0 | | `xai_generation_provider` | 10 | 0 |
| `render_pipeline` | 179 | 0 | | `media_inspection` | 9 | 0 |
| `project_split` | 155 | 0 | | `render_template_project` | 9 | 0 |
| `project_nle_export` | 111 | 1 | | `linux_media_e2e` | 5 | 0 |
| `temporal_workflows` | 101 | 0 | | `reverse_intermediate_spike` | 5 | 2 |
| `transcription_models` | 92 | 0 | | `elevenlabs_generation_provider` | 4 | 0 |
| `project_export` | 89 | 0 | | `export_profiles` | 4 | 0 |
| `codex_app_server` | 85 | 0 | | `minimax_generation_provider` | 3 | 0 |
| `generation_provider` | 64 | 0 | | `audio_sync` | 2 | 0 |
| `graphics` | 62 | 0 | | `direct_audio_generation_cancellation` | 2 | 0 |
| `agent_claude` | 57 | 2 | | `webm_capability` | 2 | 0 |
| `render_transitions_ges` | 49 | 1 | | `render_progress_ges` | 1 | 0 |
| `one_click_edit` | 43 | 0 | | `media_inspection_appkit` | 0 | 0 |
| `gpu_graphics` | 41 | 0 | | `precompose_alpha_ges` | 0 | 0 |
| `project_patch` | 29 | 0 | | `project_export_nested_effect_appkit` | 0 | 0 |
| `replicate_generation_provider` | 16 | 0 | | `project_export_prores_appkit` | 0 | 0 |
| `search_index` | 14 | 0 | | | | |
| `temporal_reconcile` | 11 | 4 | | | | |
| `google_generation_provider` | 10 | 0 | | | | |
| `openai_generation_provider` | 10 | 0 | | | | |

The last four are `harness = false` and their `main` is `#[cfg(target_os = "macos")]`; on Linux they
link and return without running a test, so they contribute 0 to every count. That is four macOS-only
lanes this host cannot exercise, not four failures.

Counting note for anyone repeating this: `--lib`'s log contains **two** `test result:` lines. The
first (`1 passed; … 787 filtered out`) is the child process that
`project::mutation::tests::project_mutation_lease_blocks_a_separate_process` spawns to run
`cross_process_lock_helper`; its output lands in the same log. The real `--lib` figure is the
second line, 781. A naive `awk` over every `test result:` line reports 2,808. Anyone
comparing this run's totals to an earlier one should confirm the earlier one excluded that line
too.

### `cargo fmt --all --check` — FAILED, exit 1 (the one real failure)

Not environmental. **`main` has been unformatted since `2cbd1147`**
("feat(agent): make Claude the preferred backend on the user's own subscription"). 14 diffs:

| File | Diff lines reported by rustfmt |
| --- | --- |
| `src-tauri/src/agent/turn.rs` | 138 |
| `src-tauri/src/settings/agent.rs` | 214, 1500, 1758, 1955 |
| `src-tauri/tests/agent_claude/auth.rs` | 213, 233 |
| `src-tauri/tests/agent_claude/real_command_turn.rs` | 83 |
| `src-tauri/tests/agent_claude/routing.rs` | 163, 198, 216, 233, 252, 270 |

All five files are in `2cbd1147`'s diffstat. The failures are line-length only — for example
`routing.rs:273` is 124 characters against rustfmt's default `max_width = 100`:

```rust
        let (_, plan) = plan_agent_turn(&preferences, true, readiness(&preferences), mcp_server()).expect("a planned turn");
```

`rustfmt 1.9.0-stable (48a229ceae 2026-09-01)`, `cargo 1.98.1`, no `rustfmt.toml` anywhere in the
repo, so this is the default profile the gate has always used. **Fix:** run
`cargo fmt --manifest-path src-tauri/Cargo.toml --all`. This audit deliberately did not, so the
failure stays reproducible; it is a one-command fix with no behaviour change.

### `pnpm rust:clippy` — passed, exit 0

All three lanes with `-D warnings`: `--workspace --all-targets`, then the `mcp-server`
`video-creater-mcp-server` bin, then the packaged
`app-runtime,custom-protocol,coreml-inspect,ges-render,gpu-render,graphics-render` `video-creater`
bin. `dist/` existed (the frontend gate's `pnpm build` wrote it). The only `warning:` line in the
log is `sherpa-onnx-sys@1.13.8: SHERPA_ONNX_LIB_DIR is not set` from a build script — expected
without the staged Linux speech worker, and not a lint.

`TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'` was needed for the third lane on this
host: `tauri.conf.json` declares eight `externalBin` sidecars and five bundled resource
directories, and only `binaries/video-creater-mcp-server-*` is staged here. That is an environmental
accommodation, identical to the one the Rust tests take.

## 2. Documentation problems found

### Corrected in this run

**`docs/product-backlog.md`, the Ranked Backlog evidence-boundary paragraph.** It listed the run
folders that went away with their worktrees and **omitted `output/gap-closure-07/`**, which is the
folder seven rows cite as their current evidence boundary (VC-001, VC-021, VC-022, VC-023, VC-025,
VC-027, VC-029), plus `output/transition-preview-parity/{speed-transitions,lottie-handles}/`, cited
by VC-015. Proof: `find /home/olhapi -maxdepth 6 -type d -name 'gap-closure-0*'` returns nothing,
`ls output/gap-closure-07` fails, and `git worktree list` holds only `main` and three unrelated
`t3code` worktrees — the `vc-native` worktree the hand-off names is gone. The paragraph now names
those folders and records the 2026-09-24 check. No row's *result* changed; only the claim about
which artifacts can still be inspected.

`Last updated:` was left at 2026-09-18 on purpose: maintenance rule 2 ties it to status, priority
or acceptance changes, and none of those moved.

### Recommended, not applied (the hand-off is outside this audit's write scope)

1. **The hand-off's header commit is stale, and so is the scope of its verification.**
   `docs/superpowers/plans/2026-09-13-editor-redesign-handoff.md` opens with "Branch: `main`, at
   `30b20e0a`" and records its whole gate run "at `30b20e0a`". There are **12 commits after
   `30b20e0a`** on `main`, and the document itself describes several of them (the gap closure 07
   native evidence, the render-lease split from `6c3a8da1`, the Claude-first rule from `2cbd1147`).
   Three of those twelve changed `src/` or `src-tauri/` — `a7c9b0f5` (2 files), `6c3a8da1` (10) and
   `2cbd1147` (18) — and four more changed `scripts/`, which `test:source-quality` gates:
   `e1418e40`, `a2e364dd`, `2de9704d`, `23125ee1`. **No recorded full-gate run covers any of
   them**, which is exactly why `cargo fmt` went red unnoticed. Recommendation:
   restate the header as "at `ab130b42`", and say plainly that the recorded gate run is at
   `30b20e0a` and the later source commits were not re-gated until this audit.
2. **"`cargo fmt --all --check` — clean, exit 0" is no longer true of `main`.** True at `30b20e0a`,
   false at `ab130b42`. The Formatting and lint section should be dated to its commit.
3. **`output/gap-closure-07/` is described as present, and it is not.** The gap closure 07 section
   says "All folders are under `output/gap-closure-07/` in the `vc-native` worktree; `output/` is
   git-ignored and exists only on this machine." The worktree and the folders are gone. The
   "Native evidence and its limits" bullet, which does list the other retired folders, should
   include `output/gap-closure-07/`. As written, a reader is told these artifacts can be
   re-inspected; they cannot.
4. **"The 17 ignored tests are all deliberate, and every one says why it is skipped" overstates by
   one.** `src-tauri/src/project/mutation.rs:276` is a bare `#[ignore]` with no reason string on
   `cross_process_lock_helper`. It is deliberate and self-evident from its body (it is the child
   half of the cross-process lock test), but it does not *say* why. Either add
   `#[ignore = "child-process helper for project_mutation_lease_blocks_a_separate_process"]` or
   soften the sentence. Every other one of the 17 carries a reason string.
5. **Stale counts, now grown.** The hand-off's `check:tooling-source` "69 files" is 85 today, and
   its `test:source-quality` "99 passed" is 117 (`23125ee1` added the export-task helper tests).
   Its Rust total of 2,788 cannot be compared to this run's 2,807 without knowing whether it
   excluded the nested helper line described above. These are staleness, not falsehood, but they
   read as current.
6. **"298 commits land between `bda87608` and `30b20e0a`."** `git rev-list --count
   bda87608..30b20e0a` is 297. The 144 + 117 + 37 split also sums to 298. Off by one under any
   reading that excludes `bda87608`.

### Checked and found true

- The four `*_appkit` / `precompose_alpha_ges` targets really are `harness = false` with a
  `#[cfg(target_os = "macos")] fn main`, and really do contribute 0 on Linux.
- 38 `--test` targets is exactly right.
- `e2e/editor-transitions.spec.ts` was added in `3014372f` and extended in `0618b34f`; no
  `test(transitions)` commit exists, as the hand-off says.
- VC-026: `src-tauri/src/codex/context.rs` is 315 lines with six sibling modules, all under 600
  lines, and `tests.rs` (5) plus `evidence_tests.rs` (4) is the claimed 9 tests.
- VC-027: `SplitProjectMutationLease` and `SplitProjectArtifactLease` both exist in
  `src-tauri/src/project/mutation.rs`, with `acquire_split_project_artifact_lease`, and both named
  tests exist in `src-tauri/tests/project_export/render_lease.rs`
  (`an_encoding_render_holds_no_project_lease`, `an_edit_during_a_render_does_not_change_its_output`).
- VC-029: `resolve_agent_backend` in `src-tauri/src/agent/mod.rs:192` really does default to
  `AgentBackendKind::Claude` when no preference is stored.
- The retained parity reports `output/transition-preview-parity/{every-kind,flattened-transitions,lut-handles}/`
  and the `output/linux-desktop-smoke-*` folders do still exist in the main worktree.

## 3. Open work

Nothing below is blocking day-to-day work. Each row says why it is open.

### Needs a machine this host is not

| Item | Why open | What closes it |
| --- | --- | --- |
| **VC-008 notarization** (`blocked`) | Developer ID notarization and stapling need an Apple Developer ID and a Mac. Neither exists here. This is the only genuinely `blocked` row, and it is correctly labelled. | A Mac plus a Developer ID certificate; then notarize, staple, and retain clean-Mac packaged evidence. |
| **macOS browser visual baselines** (redesign plan 02 Task 4) | No Mac. Every run so far, including this one, compared only `linux-x64`. | `pnpm visual:qa:refresh-manifest` and `visual:qa:browser-release` on a Mac. |
| **⌘Z single undo (VC-025)** | Manual macOS check. The Linux equivalents (real X `ctrl+z`, GTK Edit → Undo) passed 2026-09-17 and 2026-09-18. | One manual pass on a Mac. |
| **AVFoundation transitions (VC-016, `idea`)** | The exporter rejects plans containing transitions (`avfoundation_backend::reject_transitions`), so macOS falls back to GES. Not a defect — a documented deviation. | Compose transitions in the AVFoundation exporter, or decide GES fallback is the answer and close the row. |
| **Retimed audio on macOS (VC-017)** | AVFoundation retimed audio falls back to GES, and the macOS GStreamer runtime's `scaletempo` requirement (`d59b4e11`) has never run on a Mac. | Packaged macOS run of an audio clip at non-1× speed. |
| **Master on macOS (VC-019)** | Master equals Final under AVFoundation; unverified. | A packaged macOS Master export. |
| **Claude and the macOS keychain (VC-029)** | Whether a Tauri-spawned child inherits the macOS keychain login. Every Linux half is closed: all four auth states and both child-environment behaviours are covered by stub executables (`src-tauri/tests/agent_claude/auth.rs`), and a packaged Claude turn ran through the app's own command path on 2026-09-18. | Install `claude`, sign in with a `claude.ai` session, and run one packaged turn on a Mac. |
| **Four macOS test lanes** | `project_export_prores_appkit`, `project_export_nested_effect_appkit`, `media_inspection_appkit`, `precompose_alpha_ges` run only on macOS. | `pnpm rust:test:native` on a Mac; `evaluateNativeRustTestReport` already refuses a non-`darwin` report. |
| **Premiere / DaVinci round-trip (VC-024)** | Neither NLE runs on Linux, and no public reference export settles FCPXML dip-to-white, wipe effect ids, or a connected storyline anchored on a retimed primary clip — so each currently exports as a cut or a cross dissolve. `verify:nle-xml` already validates 48 corpus files against the pinned DTDs. | Open an exported XMEML and FCPXML in Premiere Pro and DaVinci Resolve and record what imports. Needs a licensed NLE on a supported OS. |
| **H.265 / HEVC Master (VC-019)** | This host has no VA-API encoder: the 2026-09-18 packaged run reported `mp4H265` unavailable, "Missing GStreamer factory: vah265enc", and the runtime build confirms `vah264dec/enc` and `vah265dec/enc` are all absent. A hardware limit, not a code gap. | A host with VA-API (or a Mac with VideoToolbox), then a packaged Master H.265 export. |
| **VC-020 "Show in folder"** | The button was found, clicked, and correctly reported failure on 2026-09-18 — this host has neither `org.freedesktop.FileManager1` nor `xdg-open`, so whether a file manager opens is not observable here. | Any desktop host with a file manager. |

### Not done, and nothing external is stopping it

| Item | Why open | What closes it |
| --- | --- | --- |
| **`cargo fmt` is red on `main`** | Introduced by `2cbd1147` and never caught, because no full-gate run covered the seven commits after `30b20e0a` that touched `src*/` or `scripts/`. | `cargo fmt --manifest-path src-tauri/Cargo.toml --all`, one commit. **Do this first** — it is the only thing standing between `main` and a green `pnpm verify`. |
| **VC-027 packaged re-measurement** | The lease split (`6c3a8da1`) is proven by two real-GES tests and by the code path, not by a second native measurement. A debug-build attempt cannot produce one: the smoke step's own `load_split_project_from_folder` probe takes the *storage* lease a render still holds start-to-finish, so a debug 1080p export exceeds WebDriver's 30 s script timeout. | Build the `.deb`, re-run the smoke's `--export-tasks` step on the **packaged** app, and expect only the editor half of the delete latency to fall. |
| **VC-027's remaining long mutation-lease holds** | Three were left alone deliberately: `caption_visual_frame_cache_in_split_project_folder` (holds across a fal.ai network round trip per frame — needs its own cache-ownership decision before it can be shortened), `write_palmier_project_package` (held on purpose, because `render_job_needs_recovery` treats an attempt-less export job as live only because of that hold), and the Temporal project-bundle activities. Plus: the storage lease itself, which is why `load_split_project_from_folder` still waits for a render. | Three separate decisions, then code. None needs anything external. |
| **VC-027 optimistic delete** | The delete is not applied optimistically in the editor, so it waits on its write. | A product decision, then a change in `src/editor/store/project-slice.ts`. |
| **VC-015 packaged transitions on retimed clips** | The Linux desktop smoke has no transition step at all. | Add one to `scripts/linux-desktop-smoke-*`, then run it packaged. |
| **VC-017 / VC-018 packaged evidence** | No smoke steps exist for audio-clip speed or Detach audio either. | Same: add the steps, run packaged. |
| **VC-019 Master + frame-rate override, packaged** | The 2026-09-18 run covered the chosen folder and chosen name at 720p High only. | Extend the export-tasks smoke step. |
| **Codex sidecar pin bump (optional)** | `@openai/codex` is pinned at 0.141.0. 0.154.0 lists `gpt-5.6-sol` and changed the app-server methods additively, but adds a `codex-code-mode-host` helper and has not been checked with a real turn. Since 2026-09-18 Claude is the preferred backend, so nothing waits on Codex. Also unrun: the Codex-backed repeat of AI flows 1 and 2 (that account hit a usage limit on 2026-09-17). | Bump the pin, run `e2e:codex` and `--agent-flows --agent-backend codex` with a real turn. Needs Codex account headroom, which is why it stays optional. |
| **VC-007 Review truth and recovery** (`in progress`, P0) | The only P0 that is neither `implemented` nor `verified`. Its next action — structured impact facts, affected IDs and ranges, post-apply preview frames, Show changes, deterministic Undo validity, concise failure recovery — overlaps heavily with work the gap closure already shipped and observed natively. The row reads staler than the code. | Re-read the row against the shipped behaviour, move what is done to `implemented`, and shape whatever genuinely remains. |

### Product direction: shaping work, not engineering work

These are older backlog entries that have never been planned. Each is open because the product
question is unanswered, not because anything is broken or missing a machine. They are listed with
their status so nobody mistakes them for regressions.

| Row | Status | The unanswered question |
| --- | --- | --- |
| **VC-004 Find the story** | `discovery` | Story-candidate output, ranking, evidence, diversity, user selection, and the line between deterministic retrieval and model judgment. |
| **VC-011 Programmable compositions** | `discovery` | The component contract, parameter schema, sandbox, deterministic preview/render boundary, versioning, and conversion to canonical timeline state. |
| **VC-005 Story map** | `idea` | The smallest representation that round-trips to a canonical EDL without becoming a second timeline. |
| **VC-006 Transcript-native editing** | `idea` | Canonical transcript operations for delete, restore, reorder, retake selection, pause shortening and navigation, including alignment-failure recovery, on the Captions transcript layout contract. |
| **VC-009 Objective automatic cleanup** | `idea` | Which detections are confident enough to apply as one atomic revision, and which stay suggestions. Shares an acceptance section with VC-006. |
| **VC-012 Story variants** | `idea` | Branch identity, shared analysis, inherited edits, divergence, comparison, export naming. |
| **VC-013 Reusable workflows** | `idea` | Structured recipes with typed inputs, ordered operations, approval gates, cost bounds, acceptance checks. |
| **VC-014 Quality scorecard** | `idea` | Which checks are deterministic, and how advisory model judgments carry evidence and confidence. |
| **VC-002 / VC-003 / VC-010** | `implemented` | All three have productization next actions rather than missing behaviour: join the existing pieces into one inspectable contract or one complete workflow. |

## 4. Are we blocked?

One thing to note first, because it is invisible from the working tree: **`main` is 311 commits
ahead of `origin/main` and 0 behind.** `origin/main` is still at `3abf089d`
("docs(linux): archive strict permissive-only runtime research", 2026-09-13), so the entire editor
redesign, the gap closure, the Claude backend and the render-lease split exist only on this
machine. That is not a blocker — `git push` needs nothing external but the remote — but it is a
single point of failure for three workstreams, and it is worth doing right after the `cargo fmt`
fix, so what lands upstream is a green tree.

**No, nothing is blocking further work.** One row is `blocked` and it is correctly labelled: **VC-008 notarization**, which needs an
Apple Developer ID and a Mac. Nothing else in the repository is waiting on an external dependency.

Everything else divides cleanly:

- **Needs hardware, not permission** — the macOS items, HEVC/VA-API, a desktop file manager, and a
  licensed Premiere or Resolve. These are unrecorded evidence, not unfinished product.
- **Just not done** — the packaged smoke steps for VC-015, VC-017, VC-018 and VC-019, the VC-027
  re-measurement and its three remaining lease decisions, and the optional Codex pin bump.
- **Not yet shaped** — the `idea` and `discovery` rows, which are product work.

**Someone can keep shipping today with no external dependency at all.** The concrete first move is
`cargo fmt --manifest-path src-tauri/Cargo.toml --all`: it is a one-command, no-behaviour-change fix
that turns `pnpm verify` from red to green, and until it lands every other change on `main` inherits
a failing gate. After that, the highest-value unblocked work is adding the four missing Linux smoke
steps (transitions on retimed clips, audio speed, Detach audio, Master + frame-rate override) and
re-running the `--export-tasks` step on a packaged build, because those close five `implemented`
rows without needing anything this machine lacks.

## 5. Sanity checks

| Check | Result |
| --- | --- |
| `TODO` / `FIXME` / `unimplemented!` / `todo!()` in `src`, `src-tauri/src`, `src-tauri/tests`, `scripts`, `e2e`, `tests` | **0 occurrences.** The redesign, the gap closure and the Claude work left none. |
| Rust `#[ignore]` | 22 attributes in the tree, 17 reachable on this host. **16 of 17 carry a reason string.** The exception is `src-tauri/src/project/mutation.rs:276`, `cross_process_lock_helper`, a bare `#[ignore]` — deliberate (it is the child half of `project_mutation_lease_blocks_a_separate_process`) but not self-declaring. The other five attributes are `cfg`-gated off Linux (the macOS keychain test) or live in a workspace crate binary (`semantic-encoder-worker`, needs a ~504 MB SigLIP 2 bundle). |
| Ignored-test breakdown | `--lib` 7 (D-Bus file manager, the lock helper, three Secret Service tests, two production model downloads at ~35 MB and ~670 MB); `agent_claude` 2 (real Claude turns, gated twice — `#[ignore]` and `VIDEO_CREATER_CLAUDE_REAL_TURN=1`); `temporal_reconcile` 4 (need a Temporal dev server); `reverse_intermediate_spike` 2 and `render_transitions_ges` 1 (30 s 1080p cost measurements); `project_nle_export` 1 (DTD validation, run by `pnpm verify:nle-xml`). |
| `test.fixme` / `test.skip` / `describe.skip` / `xit` in `e2e`, `src`, `tests` | **None.** All 72 Playwright tests and all 2,644 Vitest tests run. |
| `knip.jsonc` temporary block | **None.** The `scripts/knip-temporary-block.test.ts` policy test passes, and the file holds only two permanent entries, two permanent ignores, one ignored dependency and eleven host binaries — each with a comment saying why. |
| Files in `src/editor` over 600 lines | **None in production code.** The largest is `src/editor/services/generation-service.ts` at 549 lines. Three test files exceed 600 (`store/agent-slice.test.ts`, 721), which the policy exempts by design — `scripts/editor-source-policy.test.ts` walks `productionFiles(editorRoot)` only. The gate's own check passed in this run. |
| `t3code/*` branches | **Unrelated to this work; untouched.** `t3code/check-project-dependencies` (at `19b59f34`) and `t3code/support-custom-video-agent-keys` (at `32e81fcc`) are **fully merged into `main`** — 0 commits ahead each — and are stale worktree checkouts under `~/.t3/worktrees/`. `t3code/build-remote-connectable-frontend` is **2 commits ahead**: `4a2af883` "docs(remote): design connectable frontend" and `68ab57c2` "docs(remote): plan server foundation", both from 2026-08-02 and both **documentation only** (`docs/superpowers/specs/2026-08-02-remote-connectable-frontend-design.md`, 383 lines; `docs/superpowers/plans/2026-08-02-remote-foundation.md`, 1,149 lines). That is a separate remote/server product direction predating the 2026-09-13 redesign. Nothing in any of the three touches the editor redesign, the gap closure or the Claude backend. |

## Follow-up — what was fixed, and where this audit was wrong

Added 2026-09-24, after the audit. Everything above is left as written, as the record of what the
run found. This section records what has since been done and the four places this audit itself got
something wrong.

### Closed

| Finding | Closed by |
| --- | --- |
| §1, §3 — `cargo fmt --all --check` red on `main` | `a7e7f943`. Re-checked at `cacaa273`: exit 0. |
| §2 recommendations 1, 2, 3, 5, 6 — the hand-off's stale header, fmt claim, `output/gap-closure-07/`, stale counts and the 298/297 commit count | `c0c3fbe7`, which corrects `docs/superpowers/plans/2026-09-13-editor-redesign-handoff.md` and adds a 2026-09-24 re-run section. |
| §2 recommendation 4, §5 — the bare `#[ignore]` on `cross_process_lock_helper` | `cacaa273` gave it a reason naming the test that spawns it. All 17 reachable ignored tests are self-declaring now. |
| §3 — **VC-007** reading staler than the code | `c7099e0f`. All six pieces of its next action were re-read against the code; the row is `implemented`, and its next action is now the two gaps that are genuinely open. |

### Corrections to this audit

1. **Not all of `/tmp` was cleared.** §1 says "`/tmp` was cleared since 2026-09-18". Three gap
   closure 07 paths survive, checked on 2026-09-24: `/tmp/vc-deb-root-07/` (479 MB, 12 binaries in
   `usr/bin` plus `codex-runtime`, `render-runtime`, `sample-project` and `speech-runtime`),
   `/tmp/vc-smoke-07/app-support/render-runtime/linux-c8a1d06661198e8a68b3d0762b271d62/`, and the
   D4 export `/tmp/vc-export-target/Linux Evidence Export.mp4` at the recorded 4,939,742 bytes.
   `/tmp/vc-deb-root-06/` and the `.deb` files are gone. The conclusion §1 drew from it — that the
   staged render runtime needed no rebuild — stands on its own evidence, the 0 broken symlinks.
2. **The corrected backlog paragraph was itself wrong.** §2 "Corrected in this run" listed
   `output/linux-result-frames/` and
   `output/transition-preview-parity/{speed-transitions,lottie-handles}/` among the folders that
   went away with their worktrees. They are on disk, along with seven more parity reports, because
   `render_transitions_ges::result_frames` and `::parity` write them on every run of that always-on
   target (`src-tauri/tests/render_transitions_ges/result_frames.rs:30`, `parity.rs:184`) — this
   audit's own Rust run rewrote them. Fixed in `c7099e0f`.
3. **The "checked and found true" parity bullet is incomplete.** Nine reports exist, not three, all
   with `"status": "passed"`: `every-kind` (13 frames, max 0.00087), `flattened-transitions` (9,
   0.00087), `speed-transitions` (10, 0), `lut-handles` (6, 0), `lottie-handles` (6, 0),
   `lottie-handles-speed` (6, 0), `reverse-handles` (6, 0), `reverse-handles-speed` (6, 0),
   `reverse-spike` (7, 0).
4. **`23125ee1` did not change `scripts/`.** §2 recommendation 1 counts it among four commits that
   did. It changes `package.json` alone — the source-quality gate's own glob. It belongs in the set
   of seven ungated commits for that reason, not because it touched `scripts/`.

## Reproducing this audit

```bash
mkdir -p /tmp/vc-locks && touch /tmp/vc-locks/playwright.lock
flock /tmp/vc-locks/playwright.lock pnpm verify:frontend

node scripts/build-macos-release.mjs --prepare-mcp-sidecar --development
node scripts/build-precompose-sidecar.mjs
cargo build --manifest-path src-tauri/Cargo.toml -p video-creater-compatibility-decoder

export TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'
export VIDEO_CREATER_RENDER_RUNTIME_ROOT="$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a"
export VIDEO_CREATER_COMPATIBILITY_DECODER="$PWD/src-tauri/target/debug/video-creater-compatibility-decoder"
export CXX=c++ BINDGEN_EXTRA_CLANG_ARGS="-isystem $(cc -print-file-name=include)" GST_GL_WINDOW=dummy

cargo test --manifest-path src-tauri/Cargo.toml --lib  -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml --bins -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml --doc  -- --test-threads=1
for f in src-tauri/tests/*.rs; do
  cargo test --manifest-path src-tauri/Cargo.toml --test "$(basename "$f" .rs)" -- --test-threads=1
done

cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check   # currently exits 1
pnpm rust:clippy
pnpm test:source-quality
```

If `/tmp` has been cleared, check the staged runtime before rebuilding it:
`find "$VIDEO_CREATER_RENDER_RUNTIME_ROOT" -xtype l` must print nothing. Only rebuild with
`pnpm build:linux-media-runtime --output` if it does.
