# Architecture and performance implementation

This records the initial handoff through `fa17ddd`. Subsequent fixes and fresh gates
are in [the risk follow-up report](architecture-risk-followups-2026-10-01.md), including
reload recovery, export staging, worker capacity, preview revisions and polling quotas.

Local implementation on `feat/architecture-performance`, based on
`58ed4756a359cfcf486d60c379480e6be21c88e3`. The assessment documents and the
original tracker are preserved. Nothing was pushed, merged, published or uploaded;
external CI and branch protection were not changed. Rust remains authoritative.

## Implemented boundaries

- Remote commit acknowledgement is independent of media ticket readiness. Failed
  ticket creation leaves one installed canonical edit and recoverable media state.
- Actions compare canonical identity and revision inside the mutation lease, validate
  the complete batch, and persist transactionally. Replacement saves also reject a
  different canonical project at the same revision. Desktop callers send their
  actual base revision; legacy callers retain the optional argument.
- Progress reads stream manifest identity and bounded opened snapshot files without
  loading timelines or taking project/storage leases. Partial files, growth, symlinks
  and package replacement are covered. Full snapshot reads use the existing coherent
  project loader under its short mutation lease, without activation or the global
  storage lease. Opening/recovery remains a separate path. Undo/redo sends the additive
  `activateProject: false` save option: existing-only restoration validates identity,
  existence and revision together under mutation ownership, without session activation
  or global storage ownership. Missing packages cannot be recreated, including revision
  zero. Desktop/host/fixture semantics agree; omitted/true retains legacy activation.
- Render admission protocol 1 persists an immutable project/input, queued job and
  attempt identity before acknowledging. Execution runs outside the editor gate.
  Admission replay checks exact inputs/source revision and does not spawn another
  worker. Progress and attempt reads bypass the client mutation queue.
- Worker bookkeeping accepts only job status, render report and export artifact
  actions. It validates the entire batch and preserves the acknowledged content
  revision; actual editor writes advance it. Completion reloads canonical state and
  retains newer edits. Fallback/recovery is guarded by package and current attempt.
- A cross-process admission pin protects the admission-to-artifact-ownership gap.
  An orphaned attempt is reported interrupted; explicit guarded recovery terminalizes
  it without restarting the render. A missing result record can reconstruct a
  canonical successful outcome using the real pipeline success label, `succeeded`.
- Cleanup owns artifacts before mutation, including domain callers outside desktop
  cleanup. Render ownership remains storage → artifacts → mutation. Admission takes
  mutation plus a **nonblocking** pin and never waits for artifact ownership there.
  Storage exclusion across encoding is intentionally retained.
- RPC failures keep their codes, retry hints, request identity and phase. Queue,
  lease, read, write, job, cancel and ticket deadlines are distinct. A mutation whose
  response is lost blocks subsequent writes as uncertain; refresh obtains canonical
  state without claiming it proves the original outcome. No automatic mutation replay
  with a new request identity was added. Reconciliation checks identity, revision and
  request/session generation before clearing uncertainty.
- The remote queue admits at most 64 waiting requests and rejects new unaccepted
  work explicitly. An expired waiting write never runs later. Cancellation and safe
  reads bypass it. The desktop FIFO retains accepted edits and is unchanged.
- Lease reuse is expiry checked; tickets are session/project/resource scoped,
  coalesced, expiry aware and bounded. Only relative resource identities are retained,
  not entire canonical project closures. Active media consumers refresh on recovery.
- Project actions/progress/snapshots and render admission/status/recovery share Rust
  service boundaries. Unused `name()`-only coordinator facades were removed. The
  additive render contract is checked against Rust serde and TypeScript consumers;
  project schema version 2 and legacy synchronous render responses are preserved.
  This is a vertical migration, not a claim that every legacy RPC is typed.
- Preview plans cache measured expensive structural work with explicit size/entry
  limits and mutation-safe signatures. Filmstrip caching includes sampling width and
  bounds entries/retained representations. Canonical frame preloading bounds pending
  and loaded identities. Undo/redo keeps a combined 128 MiB serialized UTF-16 budget
  and 100-state ceiling, preserving one oversized nearest snapshot as an exception.

## Measurements

Machine, source revisions, cache state, workloads, failed/partial runs and raw log
locations are recorded in [the baseline report](performance-baseline-2026-10-01.md).
Node calculation measurements do not establish browser or native playback speed.

| Comparable workload | Before | After | Limit |
| --- | --- | --- | --- |
| 1,000 crossfade clips, Node p50/p95 | 17.07 / 24.80 ms | 1.58 / 2.18 ms | 100 observations per complete run; VM load differs |
| 100 nested crossfade clips, Node p50/p95 | 20.26 / 35.45 ms | 0.15 / 0.48 ms | Exact frame parity checked separately |
| 5,000 effect clips, Node p50/p95 | 29.18 / 32.01 ms | 10.84 / 13.23 ms | Excludes effect decoding/rendering |
| 5,000 caption clips, history heap after 125 edits | 475.9 MiB / 100 snapshots | 108.1 MiB / 14 snapshots | Real local store; undo depth is deliberately smaller |
| Controlled remote RTT 20 / 80 / 150 ms | ACK required lease + RPC + ticket: 3 RTT | First ACK 2 RTT; reused-lease ACK 1 RTT | Synthetic timers; media readiness follows independently |

All 18 final preview workloads complete; the original largest nested workload did
not complete its 30-second measurement budget, so no before/after percentile is
claimed for it. Plain/caption/many-media frame calculation did not improve in every
run. No universal speedup or native frame-rate claim is made.

The full frontend gate now performs one TypeScript pass and a bundle-only build;
standalone builds remain checked. Focused frontend/native lanes enforce exact tests
and fail empty selections. Development Cargo targets stay incremental and are never
automatically cleaned. Metrics retain latest plus 12 reports and exclude secrets.

[Helper preparation evidence](helper-preparation-2026-10-01.md) distinguishes first
materialization from warm stages. Warm Codex staging took 1,099/639 ms; the direct
helper took 471/589 ms, versus 513/499 ms for hashing source and both outputs alone.
CoW was unsupported. Additional skips/copy changes are deferred because measured
verification overhead would consume the proposed benefit.

## Verification and remaining scope

Focused regressions reproduced failures before the fixes, including commit/ticket
failure, competing writers, unsafe progress reads, cleanup under an artifact lease,
same-bucket filmstrip width changes, bookkeeping revision drift, package replacement,
missing-result reconstruction and stale reconciliation. Final gate results are
recorded in [TASKS.md](../../TASKS.md).

| Final local gate | Command | Result |
| --- | --- | --- |
| Complete frontend | `rtk pnpm verify:frontend` | Passed: source/tooling/policy/types, 282 unit files, bundle, Knip, 3 browser adapter checks, all 74 browser scenarios, 7 visual comparisons with zero mismatches |
| Rust formatting | `rtk proxy cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Passed |
| Workspace and host lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --features web-host -- -D warnings` | Passed; 58.24 s |
| MCP lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --features mcp-server --bin video-creater-mcp-server -- -D warnings` | Passed; 17.75 s |
| Packaged feature lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --features app-runtime,custom-protocol,coreml-inspect,ges-render,gpu-render,graphics-render --bin video-creater -- -D warnings` | Passed on Linux; 22.76 s; not packaged Mac verification |
| Broad portable Rust regression | `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-host --lib --timings -- --test-threads=4` | 647 passed, 0 failed, 8 ignored; build 36.44 s, tests 67.57 s |
| Portable release/runtime/gate policy | `rtk pnpm test:gstreamer-release-policy`; `rtk proxy node --test --test-isolation=none scripts/release-runtime-policy.test.ts scripts/cargo-cache-policy.test.ts scripts/native-verification-policy.test.ts scripts/native-rust-test-policy.test.ts scripts/zero-debt-gate.test.ts`; `rtk pnpm check:release-runtime-policy` | 27 + 21 passed; source policy passed |

The table's Cargo commands were prefixed with `rtk proxy env` and used
`CARGO_BUILD_JOBS=2` plus `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`
to check source without bundling helpers. Workspace lint additionally used the VM's
installed GCC compiler and standard headers: `CXX=g++` and
`BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include`. The initial
missing clang++/stdbool.h checks failed; the corrected full workspace check passed.
No operating-system package, vendor source, feature coverage or warning rule was
changed. The speech-worker build script still reports that its standalone check lacks
`SHERPA_ONNX_LIB_DIR`; the prepared Linux worker stage links its reviewed runtime.

Earlier full frontend attempts exposed generated untracked TypeScript outputs,
outdated queued-state assertions and fixture progress driven by polling count. Those
were corrected, and the source-frozen final complete gate passed. Visual baselines
and mismatch thresholds were unchanged. The GStreamer policy assertion now checks
the complete nine-stage macOS helper plan rather than the replaced shell-chain shape.
Its signing fixture checks only preflight rejection against temporary bytes; this is
not a macOS signing verification.

The final source and integration review checked atomic saves, worker revision and
attempt guards, cross-process lock ordering, replay compatibility, uncertain-outcome
reconciliation, queue admission/cancellation, history markers and cache eviction.
Review defects were corrected with regressions before slice completion. Changes are
recorded in independently reviewable Conventional Commits; the final source commit is
`8baafe2`. The local branch remains unmerged. No external CI or repository operation
requiring publication was performed.

Real Linux render tests use generated synthetic media, never private media. The
existing 150-second timeline export retained a measured 150.0233-second output while
an editor deletion committed in 634 ms. An encode completed in 120.5 ms while a
separate owner held the project mutation lease; result writes waited correctly.
These measurements concern this small 320×180 workload, not 150 seconds of wall-clock
encoding or Mac packaged performance. Two additional actual admitted-worker tests
passed: immutable 150-second MP4 plus edit/replay, and a real worker pending 32.28
seconds before encoding while 30-second editor leases renewed and reads, edits,
cancellation and takeover proceeded. Native snapshot restore also passed under a held
global storage lease and preserved session generation.

Evidence-based deferrals:

- Keep broad storage exclusion until every cleanup/cache/native writer has an
  explicit ownership contract. Safe reads and artifact cleanup were separated without
  deleting existing locks. Cross-filesystem export publication can still copy large
  outputs while holding completion mutation ownership; staging that copy needs an
  explicit publication/revalidation design. External filesystem changes can still alter
  path-named renderer inputs; immutable admission freezes project instructions, not
  source media bytes.
- Keep secondary-panel loading, dense-transition windowing and browser playback
  redesign unchanged: Node structural cost justified the implemented optimization;
  bundle size alone does not establish a cold-open bottleneck or native frame budget.
- Defer pure crate extraction. [Fresh dependency/timing inspection](native-feature-profiles.md)
  shows a useful candidate but no verified pure build boundary or faster pilot;
  no-default root tests still resolve desktop dependencies.
- Keep legacy long-running provider/workflow operations on their existing execution
  paths with operation-aware deadlines. The durable vertical slice covers in-process
  media renders. A global worker/provider migration lacks comparable evidence and
  would require service-specific recovery contracts and authorized provider testing.
- The host's generic replay cache remains bounded and in memory. Uncertain requests
  retain exact identity during a live client session; automatic replay across host
  restarts/cache expiry is unsafe and was not added. Render attempts have durable
  reconciliation. Page reloads do not preserve the client uncertainty gate.
- macOS/Xcode/signing/packaged-Mac/physical-device checks require their actual runner.
  Live Tailscale latency and production restart/takeover are unverified. The
  [CI gate map](ci-gate-map.md) names runner responsibilities; authenticated external
  required-check configuration still needs authorization and verification.
