# Architecture risk follow-ups — 2026-10-01

Local follow-up to the initial implementation at `fa17ddd6e7e81da5bdd410315e6b40d34984dbb3`
on `feat/architecture-performance`. The assessment documents and earlier evidence
remain intact. No push, merge, deployment, paid provider, private-media upload or
external CI change was performed.

The subsequent [reliability completion report](reliability-completion-2026-10-01.md)
tracks fixes and fresh verification for the remaining local gaps below. This report
preserves the earlier results and their original limitations.

## Implemented and reviewed

| Reproduced behavior before | Behavior after | Boundary |
| --- | --- | --- |
| Reload discarded an uncertain mutation guard | Credential-free markers survive same-tab, same-origin reload; later edits pause until matching canonical reconciliation | 32 markers / 24 KiB; unavailable, corrupt or full storage rejects before dispatch |
| Copy materialization deleted an unrelated partial file | Exclusive UUID stages own their files; cancellation/drop/rollback preserve replacement files | Destination-filesystem staging, inode/parent validation, no-clobber publication |
| Fifth blocked render retained another snapshot/thread | At most four admitted workers per process; excess new work is rejected before durable acceptance | Exact replay consumes no extra slot; RAII release covers errors, spawn failure and panic |
| Background preview capture advanced revision 1→3 | Capture bookkeeping preserves content revision; the next edit at revision 1 succeeds | Ordinary editor writes remain atomically revision checked |
| Snapshot/attempt/recovery operations existed but returned NotFound over HTTP | Host registry exposes those services with their existing read/write/lease classifications | Recovery still requires editor ownership |
| Visual effect catalog lacked a remote dispatcher | Remote returns the same checked Rust catalog as desktop | Read authorization; no project/editor lease needed |
| Polling exhausted the shared 60 requests/minute quota and blocked reopen | Validated reads have 300/minute, mutations retain 60/minute, cancellation has 30/minute | Four shared ordinary in-flight requests plus one cancellation slot; authorization precedes classification |

Markers contain only origin, request ID, operation, safe opaque project identities,
phase and outcome. RPC bodies, tokens, media paths and project snapshots are excluded.
Persisted metadata must have scalar outcome values; an array coercion regression was
reproduced and fixed. Corrupt metadata is preserved for recovery. Canonical refresh
checks identity, integer revision, request identity and session/mutation generation;
it cannot clear a newer marker or a still-running operation. A refresh establishes
current state, not the original RPC outcome. No automatic mutation retry was added.
Uncertain creation remains paused because catalog discovery cannot prove its outcome.

Export byte copying and syncing now happen before completion takes the project mutation
lease. Cancellation remains possible while staging. Existing storage/artifact ownership
and lock ordering remain intact. Publication under completion ownership revalidates the
package/current attempt and uses no-clobber destination selection. Existing named-export
and synchronous materialization callers use the same prepare/publish implementation.

Sampler preview bookkeeping checks package identity and active run identity under the
same mutation lease as terminal publication. It merges into current canonical state,
preserves newer edits, and retains cancellation/terminal/supersession semantics. The
native one-frame path also selects revision-preserving bookkeeping; its macOS runtime
and legacy package guard are not established by Linux sampler tests.

The limiter class comes from the validated operation descriptor, not a caller-provided
name alone. Read and mutation concurrency remains shared; cancellation gets one bounded
independent slot. Window reset preserves in-flight counts and permit drop releases the
original lane. CSRF, editor lease checks, request fingerprints and replay rules remain.
The worker limit bounds threads and retained entries, not snapshot bytes or disk history.
Rust remains authoritative; project schema version 2 and existing wire fields are unchanged.

## Verification and environment

Final gate results are recorded below and in `TASKS.md`. Workloads use synthetic media,
the repository fake Codex provider and real Rust rendering/HTTP dispatch. No provider
network calls are required. Each actual browser run receives a new isolated data root;
failed-run evidence was preserved rather than reusing the launcher's destructive reset.

Ubuntu VM: Linux 6.8.0-139 x86_64, KVM, 10 exposed vCPUs (i5-12500), 11 GiB RAM,
4 GiB swap, Node 24.18.1, Rust 1.98.1. Builds use the existing warm Cargo/helper cache
and two Cargo jobs. GCC 13 headers and `CXX=g++` are supplied to the installed native
toolchain. Approximately 80 GiB disk space remained at the verification checkpoint.
Concurrent frontend/Rust gates and VM load prevent using elapsed gate times as a
controlled speed comparison. No new latency, RSS, browser playback or native performance
claim is inferred from these correctness fixes, source structure or bundle size.

Focused RED→GREEN evidence:

- Export: old CopyOnly deleted another partial file; 20 destination tests pass after
  the fix, including cancellation, concurrent names, replaced parent/stage and byte parity.
- Admission: a fifth worker was accepted before the fix; 19 admission tests pass,
  with one child-process helper explicitly ignored at the top level.
- Capture: real sampler success/failure changed revision 1→3; cancellation became
  Failed. All six tests now pass, including a real PNG, subsequent CAS edit and package
  replacement/supersession. An invalid empty-timeline fixture was corrected separately.
- Limiter: repeated real-engine reads exhausted the ordinary budget and cancellation
  lacked a reserve. Five tests pass for finite quotas, mixed concurrency, window reset,
  RAII release and authorization. One externally terminated compilation is not evidence.
- Marker schema: an array outcome passed string coercion; the regression now rejects
  before dispatch, and all 27 recovery-storage tests pass.
- Catalog: its real RPC test first returned NotFound; final host gate covers desktop parity.

Initial actual-browser runs identified two integration defects that narrow tests missed:
capture bookkeeping advanced revision 3→5, then polling blocked reopen with a 50-second
rate-limit hint. Those runs are failed evidence. After fixes, all three actual-host browser
scenarios passed: desktop and phone edit/render/download/reconnect, and a server-committed
edit with an aborted response, same-tab reload, exact-once canonical result, retained
guard and matching-only refresh. Native artifacts and stream assertions executed.

Ignored task-owned actual-browser roots:

- `output/architecture-followup-2026-10-01/remote-browser-3d3c94ba`: initial failed gate.
- `output/architecture-followup-2026-10-01/remote-browser-59a237c0`: two passed; reopen quota failure.
- `output/architecture-followup-2026-10-01/remote-browser-85e304a1`: three passed, 1.3 min.
- `output/architecture-followup-2026-10-01/remote-browser-60d15901`: final rebuilt bundle,
  three passed, 1.0 min; host build 26.54 seconds.

Final portable Rust passed 670 tests, zero failures and eight explicit ignores, plus
10 host RPC integration tests. Build 59.37 seconds, lib tests 163.70 seconds with a
concurrent frontend workload. The frontend rerun passed 283 files / 2,793 tests in
46.11 seconds and built 2,091 modules in 5.62 seconds. The first full rerun stopped
at an existing occupied-port subprocess test's five-second timeout during compilation;
the unchanged isolated test passed in 1.13 seconds, followed by the passing full rerun.
React act warnings and the bundle chunk warning remain visible diagnostics, not native
runtime or measured performance evidence.

Exact commands (all local):

```sh
rtk pnpm exec vitest run src/lib/runtime/adapters/remote-outcome-reload.test.ts
rtk pnpm exec vitest run src/browser-visual-qa-script.test.ts
rtk pnpm verify:frontend
rtk proxy env TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' CARGO_BUILD_JOBS=2 cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-host --lib --test web_host_rpc -- --test-threads=4
rtk proxy env TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --features web-host -- -D warnings
rtk proxy env TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --features mcp-server --bin video-creater-mcp-server -- -D warnings
rtk proxy env TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --features app-runtime,custom-protocol,coreml-inspect,ges-render,gpu-render,graphics-render --bin video-creater -- -D warnings
rtk proxy env TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features web-host,ges-render --test web_host_render_followups -- --test-threads=1 --nocapture
rtk proxy env CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include VIDEO_CREATER_REMOTE_E2E_DATA_DIR=/home/olhapi/projects/video-creater/output/architecture-followup-2026-10-01/remote-browser-60d15901 pnpm exec playwright test --config playwright.remote.config.ts
rtk proxy cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
rtk git diff --check
```

Clippy passed with `-D warnings` for the workspace/all-targets default-plus-host
profile (57.73 seconds), MCP binary (19.45 seconds), and packaged-app Linux feature
composition (23.80 seconds). The workspace build reports that sherpa-onnx is not linked
without its dedicated worker environment; this is not speech-worker packaging evidence.

Source-frozen `rtk pnpm verify:frontend` also passed all 74 browser scenarios (4.7 min),
three browser-adapter tests, seven Linux visual comparisons with zero mismatches,
source/tooling policies, ESLint/TypeScript and Knip. Rust formatting and diff whitespace
checks passed. Independent reviewers made no edits during final gates.

The final HTTP/native follow-up rerun passed both tests in 37.61 seconds after a
56.62-second build. Its pending worker lasted 33.236 seconds while real authenticated
operations proceeded. Native rendering, stream checks, immutable admission and durable
replay executed; there were no unavailable-runtime early returns. Earlier in this
follow-up, six native named-export tests passed (14.26 seconds) for MP4/WebM Master,
30 fps, destination preservation, cancellation and invalid destination rejection.
OpenH264 emitted initialization diagnostics in native tests/browser runs; final artifacts,
streams and bytes were still explicitly checked and passed.

The final actual-host browser rerun passed all three scenarios against the rebuilt final
bundle in 1.0 minute. Its host build took 26.54 seconds. Review found no newly introduced
scoped blocker. The seven local implementation/test commits are:

- `ee2cdbc8`: reload-safe uncertainty.
- `3c6e579c`: cancellable owned export staging before completion mutation ownership.
- `fc7884cb`: admission capacity before durable acceptance.
- `e4b6caf3`: revision-preserving canonical capture with sampler identity/run guards.
- `015050f4`: checked host operations/catalog and real HTTP/native follow-up tests.
- `019e59c3`: bounded read/mutation/cancellation quotas and cancellation reserve.
- `934c0248`: browser lost-response/reload/reconnect regression coverage.

The final tracker/report commit follows these slices. The local branch and unintegrated
caches remain available; no worktree, branch or user media was deleted.

## Remaining risks and justified deferrals

- Global storage ownership still spans encoding. Immutable admission freezes canonical
  project/EDL inputs, not external media bytes. Changing these boundaries needs a storage
  and source-lifetime contract; locks were not removed to improve latency.
- The real HTTP worker remained pending for 33.236 seconds while renewal, edits, reads,
  cancellation, recovery and takeover proceeded. It was held before encoding. Neither
  that test nor short native renders prove more than 30 seconds of actual encoding.
  Representative long-encode performance/device testing remains unverified.
- Tower HTTP integration uses real routing, cookie/CSRF authorization and dispatcher
  with simulated trusted proxy identity; browser gates use the explicit debug E2E host.
  Live Tailscale Serve, production restart, macOS/AVFoundation, signing, packaged Mac and
  physical-device verification require the corresponding environment and authorization.
- Recovery markers survive same-tab/same-origin reload only. New tabs, cleared browser
  storage and different host origins are outside this guarantee. Safe cross-restart
  replay and uncertain creation need durable outcome/tombstone protocols; guessing a
  mutation failed or retrying under a new request identity remains unsafe.
- Generic RPC replay has the existing 512-entry/five-minute in-memory bound but no
  response-byte cap. Durable attempt input/result history has no retention-byte cap.
  Session limiter entries and admission pin files also lack lifecycle retention ceilings.
  Adding eviction without a replay/acknowledgement retention contract could discard
  accepted outcomes or active artifacts. The four-worker limit is not a byte budget.
- External parent/stage replacement is detected and conservatively preserves forensic
  files. Crashes may leave hidden staging files. Publication and rollback still have a
  narrow external-writer TOCTOU window and are not fully directory-FD-relative operations.
  Non-Unix identity and cleanup behavior is unverified and conservative; no hostile
  filesystem or all-platform guarantee is claimed.
- Read capacity remains finite and shared by a session; sufficient multi-tab polling can
  still hit 300/minute. Typed retry hints remain visible. No automatic mutation retry
  or quota removal was introduced.
- Helper-copy optimization, pure Rust crate extraction, additional UI lazy loading,
  browser/native playback tuning and wider legacy provider/workflow admission remain
  evidence-dependent deferrals from the original report. No framework/persistence rewrite
  was justified. The local CI gate map remains available; external configuration is unchanged.

Independent read-only reviews and primary integration review cover revision ownership,
request/session identity, cancellation, replay, capacity release, export publication,
package replacement, lock ordering and file/wire compatibility. This report supplements
[the initial implementation report](architecture-performance-implementation-2026-10-01.md),
whose original measurements remain historical rather than newly rerun benchmarks.
