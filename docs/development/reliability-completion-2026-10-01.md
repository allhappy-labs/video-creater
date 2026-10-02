# Reliability completion — 2026-10-01

Continuation of the architecture plan and risk follow-ups, starting from clean
`604a38949811d9b2fdc28803f762fd25188308f7` on `feat/architecture-performance`.
The assessment documents and original measurements are preserved. Nothing has been
pushed, merged, deployed, uploaded, or changed in external CI.

## Implemented contracts

Rust still owns canonical validation, content revisions, project mutation/storage
leases, EDL inputs, accepted jobs and artifacts. The changes close the remaining
local recovery and retention gaps without removing those locks.

| Boundary | Contract |
| --- | --- |
| Durable RPC outcomes | Before mutation dispatch, fsync a compact Pending receipt keyed by hashed trusted identity and original request ID. Restart changes Pending to Interrupted, never Failed. Exact body fingerprints reject conflicting replay; receipt recovery never redispatches accepted work. |
| Creation commit witness | Persist a server nonce before dispatch; create a project with its exclusive package identity. A complete validated manifest at that exact identity proves a committed creation after interruption. No manifest means continued uncertainty. |
| Browser recovery | Negotiate additive `outcomeProtocol:1`; restore outstanding outcomes after new session/tab or cleared local storage. Canonical refresh checks project/revision/session/guard identity and confirmed acknowledgement before resuming edits. Reads and Stop remain usable when recovery is unavailable. |
| Wire compatibility | Base RPC protocol remains 1. New clients send `x-video-creater-outcome-protocol:1` and preserve typed `outcome_unknown` / `outcome_expired`. Older decoders receive HTTP 502 text for those outcomes, preserving their existing unreadable-response uncertainty fence instead of misclassifying an unfamiliar code as rejection. Original nonempty request IDs up to 128 UTF-8 bytes remain accepted. |
| Journal retention | Ordinary records: 16,384 / 16 MiB; separate Stop reserve: 1,024 / 4 MiB. Pending/Interrupted reserve 4 KiB each. Only acknowledged, known terminal timestamped identities older than 30 days are pruned. A missing expired identity cannot become new work. Legacy and uncertain records are retained; capacity rejects only unaccepted requests. |
| RPC/cache/limiter | Cache: 64 MiB serialized responses, 8 MiB each, compact replay receipts for oversized responses, 512 ordinary entries plus 64 reserved Stop entries / five minutes. Durable journal fences cache expiry. Limiter: 2,048 entries; only expired idle windows are pruned, preserving active ordinary and cancellation permits. |
| Render history | Input: 64 MiB each / 256 MiB across four admitted workers. Package metadata: 512 MiB / 1,024 attempts, including 64 MiB future result reservations. Protocol-2 terminal receipts omit duplicated project snapshots and retain input fingerprint, source revision, package and attempt identities. Protocol-1 records still read. |
| Render lifecycle | Reserve before durable admission; replay precedes capacity checks. Release snapshot/result memory before the worker permit, then unlink only the owned terminal pin under mutation ownership. Active, superseded, malformed, replacement and unrecovered state is preserved. Explicit terminal recovery compacts eligible crash-left records. |
| Sessions | At most 1,024 live credentials / 2 MiB serialized file. Prune expired/revoked sessions at bootstrap and issuance while preserving exact expiry. Issue/revoke/rotate/CSRF changes persist before replacing in-memory authorization. Failed persistence preserves existing credentials. |
| Export publication | Exclusive private stages contain durable ownership witnesses and hold a directory lock through prepare/publication. Retained directory/file descriptors anchor no-clobber publication and rollback after parent replacement. Quarantine precedes conditional deletion; restore replacements or preserve forensic evidence. Recover only verified dead stages. |
| Expired project identities | Authenticated GET410 attests only strictly-aged timestamped identities absent across all principals. Explicit independent canonical identity/read and CSRF ACK rechecking absence/age under the journal mutex allow clearing only the matching project guard; no success/failure is inferred. Retained/foreign/malformed/future/legacy/unavailable cases remain guarded. |
| Recovery I/O | Journal lookup/list/ack and creation-witness reads run on blocking workers under finite session read permits plus 64 global slots, so slow recovery disk I/O does not block the Tokio event loop. |
| Read semantics and deadlines | All 93 Rust remote-operation classifications are checked against the browser policy. Native preview reads retain long deadlines without mutation markers/ACKs. Exact migration removes only the four historical false read markers; accepted edit guards and the existing FIFO/Stop bypass rules remain intact. |

A canonical mutation remains committed if media authorization or outcome-receipt
publication later fails. Panic recovery wakes cache waiters with a retained uncertain
receipt; it does not remove the replay fence. No automatic mutation retry with a new
identity was introduced. Stop has separate admission and journal capacity.

## Evidence and reproducibility

Ubuntu x86_64 VM, Linux 6.8.0-139, Rust 1.98.1, Node 24.18.1, existing warm
Cargo/helper caches and two Cargo jobs. Native compilation uses GCC 13 headers and
`CXX=g++`. The browser host and long-encode tests use synthetic media; the browser
host uses the repository fake Codex fixture. Linux media tests and packaged desktop smoke also
use the retained Edison speech/sample fixtures. No private media or paid provider
calls were used. These are local correctness checks, not live Serve or production proof.

Focused regressions reproduced before fixes include duplicate creation after host
restart, absent authenticated receipts, panic-stranded replay waiters, missing Stop
receipts, unbounded cache/limiter population, moved-parent/dead export stages, foreign
pin adoption, failed-persistence authorization changes, package-budget races, legacy
Pending reservations, expired startup session population, and terminal recovery leaks.
The old-client decoder and full server-only browser-storage recovery cases were also
reproduced before their fixes. Incorrect fixture data and sandbox spawn failures are
recorded separately from valid RED evidence.

Comparable measurements describe serialized retention and tooling, not application RSS:

| Workload | Before | After |
| --- | ---: | ---: |
| Same render input fixture | 2,221 bytes | 563 bytes |
| Same completed result fixture | 3,569 bytes | 921 bytes |
| Ten 7 MiB RPC result payloads, retained serialized cache | 73,400,870 bytes | 58,721,122 bytes |
| One 9 MiB RPC payload, retained replay response | Full oversized result | 213-byte receipt |
| Occupied-port rejection, warm machine, three subprocess samples | 2,236 / 2,208 / 2,249 ms | 36 / 35 / 34 ms |

The final native VP8 encoder rerun advanced for 32.2723 seconds on one inherited allowed CPU:
progress .003504→.139227, output 132,736→1,169,718 bytes, 17 observed advances.
Authenticated renewal/read/edit/exact replay/live recovery/takeover/cancellation ran
alongside it. A separate SIGKILL/reap/restart test observed interrupted state without
redispatch, rejected missing CSRF, recovered explicitly and validated a subsequent
2-second MP4 with streams. These prove liveness/recovery for this workload, not native
throughput or device performance. The four host/native cases passed in 101.08 seconds;
the five Linux media cases passed in 7.29 seconds, after a 90-second warm compilation.

## Final gate ledger

Completed commands on this wave's source:

| Gate | Command / profile | Result |
| --- | --- | --- |
| Frontend | `rtk proxy pnpm verify:frontend` | Passed on final implementation: source/tooling policies, both TypeScript configurations, 288 files / 2,952 unit tests, bundle, Knip, 74 fixture browser scenarios (3.9 minutes) and seven visual comparisons; zero visual mismatches. |
| Portable Rust | `cargo test --no-default-features --features web-host --lib` with four test threads; 13 explicitly selected host targets | Passed: 726 library tests, nine explicit ignores, 55 host boundary tests. Warm compilation 110 seconds; library execution 178.84 seconds alongside frontend checks, not a performance comparison. |
| Native exports | `cargo test --features web-host --test project_export -- --test-threads=1 --nocapture` | Passed: 91 cases, including six real named export checks, 155.91 seconds. macOS-specific paths remain unverified. |
| Native media/recovery | `cargo test --features web-host --test linux_media_e2e --test web_host_render_followups -- --test-threads=1 --nocapture` | Passed: five Linux media and four HTTP/native cases; the child helper is exercised by parent tests. |
| Release policies | Eight selected release/runtime/cache/native/Linux/host Node policy files | Passed: 54 tests. |
| Formatting | `rtk proxy cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Passed. |
| Clippy | Workspace/all-targets defaults plus web-host, MCP, packaged desktop feature composition, all with `-D warnings` | Passed: final host workspace rerun 18.74 seconds; MCP 20.91 seconds; desktop 21.01 seconds. |
| HTTP representation correction | `cargo test --no-default-features --features web-host --test web_host_request_outcomes -- --test-threads=1` | Passed: 10 tests after boxing the endpoint authentication error to satisfy Clippy. |
| Journal final correction | `cargo test --no-default-features --features web-host --lib web_host::request_outcome -- --nocapture` | Passed: 18 tests, including injected mid-scan EIO and stale-errno clean EOF; 0.20 seconds after 38.36-second warm compilation. |
| Remote integration controls | Seven transport/policy/recovery Vitest files | Passed: 221 cases after the real-host read-classification correction, including historical marker migration and long-read deadlines. |
| Actual-host browser | `rtk proxy env CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include VIDEO_CREATER_REMOTE_E2E_DATA_DIR=<new-isolated-root> pnpm exec playwright test --config playwright.remote.config.ts` | Passed: four scenarios, 40.0 seconds. Real HTTP/Rust rendering, synthetic WebM and fake Codex; desktop/phone editing/reconnect/render/download, same-tab reload and new-tab cleared-storage lost-ACK recovery without replay. |

Cargo commands above use `--manifest-path src-tauri/Cargo.toml`, two build jobs,
`TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`; native/Clippy commands
also use `CXX=g++` and
`BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include`.
The portable test lane selects supported host targets explicitly: unrestricted
`--lib --tests` also selects binaries requiring `app-runtime`, and is not a supported
portable all-target gate.

Independent review identified a fail-open `readdir` error path in journal loading;
valid RED returned a partial receipt list after injected EIO. The corrected loader
clears errno immediately before each read and discards partial listings on error.
The complete journal boundary, host workspace Clippy, formatting, primary source review
and a second independent review passed after the correction.
The first actual-host run exposed four browser/Rust read-classification mismatches.
Valid policy RED (four failures), transport RED (four failures), and historical-marker
migration RED (four failures) preceded the fix. A second real-host run reached successful
reconciliation but asserted empty storage while legitimate filmstrip work was still pending;
the tests now wait for the exact settled marker set, including the preserved unrelated guard.
Recovery scenarios also use distinct project names to avoid opening an earlier fixture project.
Both failed runs and their traces are retained under the reliability output directory.

The broad portable run preceded the final journal I/O correction; the 18 journal tests
and workspace/all-targets Clippy were rerun after it. No full portable rerun after that
correction is claimed. Intermediate command-selection errors, sandbox EPERM failures,
occupied-port timeouts, and corrected Vite partial-cache browser failures are not passing
evidence. The release helper build below establishes speech helper/library packaging;
Clippy's earlier speech-worker link-environment diagnostic alone does not.

## Packaged Linux acceptance

Both packages were built from clean source commit
`7b5775623c26bd2685534b363dbdc9a98ee54fc6`. The subsequent handoff commit changes
documentation only. No package was installed, published or deployed. Existing host
package artifacts and the shared `/tmp/video-creater-editor-project` were preserved.

| Gate | Fresh result |
| --- | --- |
| Linux desktop release | Passed `pnpm release:linux`, feature profile `app-runtime,custom-protocol,ges-render,gpu-render,graphics-render`. All seven sidecars plus runtime/sample resources present; 71 package entries, zero missing payloads and zero denied ELF dependencies. |
| Extracted packaged smoke | Passed 17 checks, zero failures; one native-keyboard case skipped in this first run and exercised by the next run. Real WebKitGTK playback, import, MP4 H.264/AAC export, DeepFilterNet3 denoise, XML/project package export, webview undo and loaded-library audit. |
| Native keyboard/menu follow-up | Passed all six checks, zero skips/failures. Actual X Ctrl+Z restored exactly one edit, then GTK Edit → Undo restored the next using F10/Right/Down/Return. |
| Headless-host package | Passed `pnpm build:remote-host`, default features plus `web-host`; 282 files including manifest. All 281 listed file sizes/hashes independently verified; no remaining symlinks. Staged Codex initialized in an isolated home and was reaped. |
| Isolation/cleanup | Both desktop runs restored the original sample inode 3801567. No owned app/host/WebKit/driver/Xvfb processes or display-94 lock remained. Test-created samples and extraction roots are retained separately for forensics. |

The desktop MP4 measured 8.023333333 seconds at 1280×720, with H.264 video and AAC
audio streams, 4,939,742 bytes and a succeeded pipeline report. This is artifact
validation, not a controlled rendering-speed comparison. Software rendering/fake audio
were used on Xvfb; hardware/display/audio-device performance is unverified.
Semantic search correctly reported `modelNotInstalled` in the isolated data directory;
its status contract passed, but model inference did not run. Speech payload presence
does not establish model inference or live credential/provider behavior. Portal/PipeWire
warnings from the headless session remain in the logs.

The first offline release attempt failed at ONNX linking: ort-sys interprets
`CARGO_NET_OFFLINE=true` as skipping binary setup before consulting its binary cache.
The documented explicit `ORT_LIB_PATH` resolved this without source changes or downloads.
The cached directory key matches the pinned ort-sys distribution hash
`e454f710f8a49f53aa5b4ff51e3454ae1835777e431c6c35c5255ce6f205fd68`.
The cached static library itself has SHA-256
`0bb8a9982b44df690195c2c34b75ca791c3b9f20070b8cecbd8f50c6264dd2e2`;
the original distribution archive was not downloaded/reverified during this offline run.
Successful release command on this VM:

```sh
rtk proxy env CARGO_NET_OFFLINE=true VIDEO_CREATER_OFFLINE_BUILD=1 ORT_LIB_PATH=/home/olhapi/.cache/ort.pyke.io/dfbin/x86_64-unknown-linux-gnu/e454f710f8a49f53aa5b4ff51e3454ae1835777e431c6c35c5255ce6f205fd68 CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include pnpm release:linux
rtk proxy env CARGO_NET_OFFLINE=true VIDEO_CREATER_OFFLINE_BUILD=1 CARGO_BUILD_JOBS=2 CXX=g++ BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include VIDEO_CREATER_RENDER_RUNTIME_ROOT=/home/olhapi/projects/video-creater/output/reliability-completion-2026-10-01/runtime-for-host-7b5775623c26-1790882008374 pnpm build:remote-host
```

The second command uses an owned unique copy of the current reviewed runtime so the
packager's fixed output selection cannot overwrite earlier packages. Smoke reproduction
is retained in ignored `output/reliability-completion-2026-10-01/run-packaged-smoke.py`:
it verifies the audited package SHA, extracts into a new directory, rejects occupied
desktop/driver resources, renames the existing shared sample aside, isolates app support
and XDG data, runs `dbus-run-session -- node scripts/linux-desktop-smoke.mjs`, reaps owned
processes and restores the original sample. `VC_SMOKE_NATIVE_MENU=1` selects the second
run with cached xdotool/gst tools. Use the same preservation steps on another machine.

Ignored artifacts on this checkout:

- Desktop audit: `output/linux-release/7b5775623c26/report.json`.
- Retained desktop package: `output/reliability-completion-2026-10-01/Video_Creater-7b5775623c26-linux-amd64.deb`;
  SHA-256 `cf8a4f5ae1e9e9fb43daf366625e816b5c9b183af17f85a20bc290933988e50d`.
- Desktop smoke: `output/reliability-completion-2026-10-01/vc-reliability-smoke-dxaji64q/`;
  native menu: sibling `vc-reliability-smoke-il5z465a/`, each with evidence, screenshots
  and isolation receipts.
- Host package: `output/remote-host-package/runtime-for-host-7b5775623c26-1790882008374/`;
  independently checked receipt in `output/reliability-completion-2026-10-01/remote-host-package-receipt.json`.
  Host binary SHA-256 `0c982ebbe474b30297a7670a8fc968a4eec43612f05b6ae4c6e8b6e38870075e`.

Raw frontend/native/Clippy/browser/release logs remain under
`output/reliability-completion-2026-10-01/`; these ignored artifacts do not automatically
exist in another checkout. The final source/diff and documentation received independent
review; the overly broad synthetic-media claim was corrected against the actual fixtures.

## Platform acceptance and remaining limits

All justified locally executable implementation and Linux package/smoke gates are
complete. macOS/Xcode/signing/AVFoundation/packaged-Mac and physical devices require
their actual environments. Ubuntu cannot establish them. On an authorized Mac, start
with `pnpm release:macos:preflight` and `pnpm verify:native:release`, then perform signed
packaged acceptance and the packaged VC-027 latency workload; Linux observations cannot
substitute for those checks.

Fresh read-only Tailscale discovery found Running/online and an existing Serve mapping
from 4778 to 4777, whose local health target refused connection. No service or Serve
configuration was changed. Live authenticated Serve and deployed restart require
an authorized running host; debug E2E routing does not establish that result.
The remaining live-host acceptance is to verify the exact deployed source SHA, authenticated
MagicDNS `/healthz` route, lost-response recovery, ownership renewal/takeover/Stop and
restart recovery through the existing Serve route. Starting/replacing the deployed
service or changing Serve requires authorization; the local packages do not authorize deployment.

Global storage ownership still protects artifact lifetimes through encoding. External
source media bytes remain mutable. A safe lifetime/cleanup contract is required before
shortening that ownership; no lock was removed for latency. Descriptor publication is
implemented on Linux/macOS, with macOS execution unverified; other platforms fail
closed. Same-UID hostile writers able to mutate private quarantine directories are
outside cooperative lock exclusion. Unsupported birth-time witnesses, corrupt markers
and failed restores preserve evidence. Discovery is bounded to 4,096 entries / 64
stages per preparation and can leave additional stages for later inspection.

Journal/metadata limits are logical serialized-byte limits, not RSS, total media disk
usage, or peak atomic-file duplication. Unknown and legacy outcomes cannot be safely
evicted; sustained pressure can intentionally reject new work. Uncertain creation
without a commit witness remains paused, including a stale creation marker whose
terminal receipt has already been pruned. Expiry attestation supports explicit project
reconciliation; it does not fabricate a creation outcome. Cross-origin recovery requires authenticating
to the same host principal; local credential-free markers never carry credentials.
Absent legacy identities from before durable journaling cannot receive an expiry attestation;
their unknown mutation guards stay closed rather than infer an outcome from missing records.

Original helper-copy, pure-crate extraction, additional UI loading and playback tuning
remain measurement-dependent deferrals. This wave supplies correctness/retention
contracts, not evidence for a framework, renderer or persistence rewrite. External CI
and branch protection remain unchanged; use the existing checked gate map before any
authorized runner configuration.

See [approved reliability plan](../plans/2026-10-01-reliability-completion.md),
[earlier risk follow-ups](architecture-risk-followups-2026-10-01.md),
[initial architecture implementation](architecture-performance-implementation-2026-10-01.md),
[native feature profiles](native-feature-profiles.md), and [CI gate map](ci-gate-map.md).
