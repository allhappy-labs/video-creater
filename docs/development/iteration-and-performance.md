# Development iteration and performance evidence

Use `pnpm dev` for desktop development. It prepares the same ordered native helpers
as before, then runs Tauri. `pnpm dev:web-runtime` is the internal browser runtime;
its fixture flows do not verify a native or packaged application.

## Focused feedback

Run these from the repository root; shell examples use the repository's `rtk` policy.

```sh
rtk pnpm verify:dev:frontend src/lib/timeline-preview.test.ts
rtk pnpm verify:dev:frontend src/lib/runtime/adapters/remote-transport.test.ts src/lib/runtime/backend-client.test.ts
rtk pnpm verify:dev:native project::split::tests::fully_qualified_test_name
```

The frontend lane runs both TypeScript checks and the selected existing test files.
Missing files and an empty test selection fail. It does not bundle, run browser
acceptance, or verify a native runtime. Use an actual fully qualified Rust test name
in the native example: the runner runs each name separately with `--lib --exact`
and requires exactly one passing, non-ignored test. A typo cannot pass as zero tests.
This lane uses the default native feature set, the validated `src-tauri/target`
development cache and `CARGO_INCREMENTAL=1`; it does not check the whole workspace.
It requires 20 GiB free and never automatically deletes a development cache.

`pnpm verify:native:fast <exact-name>` remains the bounded verification lane, with
workspace checking, `src-tauri/target/verify` and incremental compilation disabled.
Its exact filters now also fail on zero matches. The release lane retains all three
Clippy feature profiles, the serial suite and the required Mac-native tests. See
[the gate map](ci-gate-map.md) for platform requirements.

`pnpm build` still includes TypeScript checking. The full frontend gate runs `lint`
once and then `build:bundle`, eliminating its duplicate frontend TypeScript pass
while preserving all source, unit, browser, unused-code and visual gates.

## Opt-in measurements

```sh
rtk pnpm perf:iteration --lane frontend --repeats 2
rtk pnpm perf:iteration --lane focused-frontend --repeats 2
rtk pnpm perf:iteration --lane helpers-linux --repeats 2
rtk pnpm perf:preview
rtk pnpm perf:preview --sizes 100,1000 --scenarios transitions,nested --samples 100 --max-workload-ms 60000
rtk pnpm perf:history
```

The helper lane performs real preparation and may build/download native runtimes.
Run it only when preparing desktop development is intended; coordinate with other
Cargo work. `helpers-macos` requires macOS. The preparation entrypoints retain a
span for each original helper stage; no new fingerprint-based skipping is enabled.
Stage measurements and invalidation inputs must justify that later change, including
source, lockfiles, compiler/target, flags, runtime manifests and packaging/copy inputs.

Reports are written to ignored `output/build-metrics/`: each lane keeps a latest
JSON and the newest twelve timestamped reports. Older files belonging to that exact
report family are removed; unrelated files and caches are preserved. Reports record
the source SHA, dirty-state marker, allowlisted tool versions, CPU/memory/platform,
feature profile, existing-cache state, phase durations, exit codes and status.
They deliberately exclude arbitrary environment variables, provider configuration
and freeform process arguments. Native verification keeps its existing latest path
and now retains the same bounded history.

The default iteration label is `warm-existing`: dependencies and caches are retained.
`--cache cold-user-isolated` is only a label for an independently prepared isolated
checkout. The tool does not manufacture a cold baseline by clearing caches.
Compare repeated runs on the same source/profile/machine without concurrent heavy
work; failed runs and sandbox launch failures are not passing performance evidence.

The preview probe uses deterministic 100/1,000/5,000-clip fixtures for plain clips,
dense crossfades, nested timelines, captions/transcript metadata, many media assets,
and preparation-required effects. It loads the real TypeScript frame calculator
through Vite SSR without a listening server, takes 20 warmups and requests at least
100 observations per fixture, and records actual dimensions, p50/p95 calculation
spans, serialized bytes, sampled heap, periodic collected heap and post-release heap.
First-calculation and maximum warmup spans expose initial structural planning cost.
There are no provider calls or generated media files.

Each fixture has a default 30-second measurement budget checked between calculations.
A slow individual frame can exceed it. Incomplete fixtures are explicitly marked
`partial-time-budget`, with actual/requested counts; their percentiles are not
accepted as complete 100-observation baselines. Increase the budget deliberately
for pathological scenarios. GC measurements require `--expose-gc` (included in the
package script); the report records whether collection was available.

Node frame/heap results exclude React, browser paint, media decoding, native rendering,
network, project persistence and editor undo/history. Effect fixtures measure the
preparation-required calculation path, not prepared playback. Use
`visual:qa:preview-performance` for its existing decoded-frame browser fixture,
browser interaction captures for UI claims, real Linux desktop for native claims,
and packaged macOS/WebKit before claiming Mac performance. Rich browser interaction,
transport latency, native startup/export and stable editor-heap baselines remain
separate evidence to collect; no Node threshold stands in for them.

The history probe uses the real Zustand project slice with an empty local project
directory and no backend writes. It measures plain, caption and many-media projects
at the same three sizes through 125 edits, undo and redo, with collection samples.
Reports distinguish UTF-8 serialized snapshots, the policy's conservative UTF-16
representation size, and observed Node heap. Available undo depth varies with the
100-snapshot/128-MiB logical budget; one oversize snapshot is explicitly retained.
This measures project-history retention, excluding browser/React, selections, jobs,
media caches and persisted undo. It is not an exact editor heap limit.

The frame calculator retains at most two structural plans, each with a signature
of at most 2,097,152 characters and 100,000 expanded structural units. Only nested,
transition and effect inputs use this cache; cheap plain inputs avoid serialization.
Source identity and content signatures invalidate replaced and in-place edited data.
Playhead, source time, transition progress and effects still evaluate per frame.
The two retained source snapshots remain reachable after fixture release until
eviction; post-release Node heap therefore includes these bounded plans.

On this VM, sandboxed Node child processes have returned `status: 0` alongside an
`EPERM` launch error with empty output. New runners check both fields. A Node
`--test` file-wrapper pass without named subtests is insufficient evidence here;
rerun affected subprocess checks through the authorized execution boundary and
inspect real named-test counts and output. `test:development-policy` disables process
isolation to make its local named tests visible, but tests that launch subprocesses
still need working process permissions.
