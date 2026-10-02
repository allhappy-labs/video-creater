# Initial iteration and preview calculation evidence

Collected on the Ubuntu VM on 2026-10-01: x86_64, Intel i5-12500, 10 logical CPUs,
12,534,661,120 bytes RAM; Node 24.18.1, pnpm 10.30.3, rustc/Cargo 1.98.1.
No user development caches were cleared, paid providers called, or native packaging
performed. All figures are local observations, not product performance guarantees.

The original frontend source at `2a04a87611c56e1ef9aa35586dd4098d5bec8125` was
copied to a temporary source snapshot with the existing dependency tree linked in.
This insulated the before measurements from concurrent implementation work. The first
snapshot run had empty `dist`; the second retained its build outputs. No process-level
cold dependency baseline was manufactured. Concurrent VM activity limits comparisons.

| Original-source command | First snapshot run | Repeated run | Evidence |
| --- | --- | --- | --- |
| `pnpm build` | 20.53 s | 13.16 s | Both passed; standalone checked bundle |
| `pnpm lint` | 11.28 s | 10.07 s | Both passed; both TS configurations |
| Three selected runtime/preview unit files | 1.39 s | 1.30 s | Both passed; 87 tests in 3 files |

Raw logs and report are retained locally under
`output/performance/snapshot-before-2026-10-01T11-00-08.982Z/`.
The early `before-2026-10-01T10-44-44.964Z` report is explicitly invalid: sandboxed
subprocess launches returned `EPERM` alongside status zero and empty logs.
The incomplete snapshot without `index.html` and moving-workspace TS failure reports
are failed runs, not accepted baseline timings. New maintained runners fail launch errors.

The rich Node preview report is
`output/build-metrics/preview-calculation-2026-10-01T10-57-10.134Z-710950.json`:
source SHA `6cf8eb86ecc39e3cd4c9d15d3043084e8fb554de`, dirty workspace marker,
existing caches and `node-preview-calculation-only` profile. The preview calculation
source was unchanged by concurrent remote/native work at collection. Each complete
row has 20 warmups and 100 observations, alternating clip interiors and cut centers.

| Scenario / clips | p50 / p95 calculation | Sampled peak heap | Completeness |
| --- | --- | --- | --- |
| Plain / 5,000 | 1.39 / 2.94 ms | 67 MiB | 100/100 |
| Crossfades / 1,000 | 17.07 / 24.80 ms | 66 MiB | 100/100 |
| Nested crossfades / 100 | 20.26 / 35.45 ms | 388 MiB | 100/100 |
| Captions / 5,000 | 1.80 / 5.19 ms | 99 MiB | 100/100 |
| Many media / 5,000 | 2.24 / 3.44 ms | 105 MiB | 100/100 |
| Preparation-required effects / 5,000 | 29.18 / 32.01 ms | 98 MiB | 100/100 |
| Crossfades / 5,000 | 320.34 / 470.92 ms | 100 MiB | Partial: 67/100, 30 s budget |
| Nested crossfades / 1,000 | 266.52 / 455.89 ms | 977 MiB | Partial: 82/100, 30 s budget |
| Nested crossfades / 5,000 | Unmeasured | 1,569 MiB during warmups | Partial: 0/100, 30 s budget |

Partial percentiles are observations with insufficient samples, not accepted complete
baselines. The Node heap generally returned near its pre-fixture level after release
and explicit collection. This points to repeated planning/allocation cost for nested
and transition inputs; it does not demonstrate a retained editor-history leak.
Peak heap is sampled after calls and can miss allocations within a call.

This probe excludes React, browser painting, actual media decoding, native rendering,
remote round trips and persistence. Effect clips request canonical preparation but
do not decode prepared outputs. Browser/native interaction and memory stability require
separate measurements. At this initial baseline, helper and native incremental timings
were pending. Subsequent [helper measurements](helper-preparation-2026-10-01.md) and
[native feature inspection](native-feature-profiles.md) support keeping additional
helper skips and pure crate extraction deferred. See
[the reproducible tooling guide](iteration-and-performance.md).

## Preview structural planning change

The final maintained report is
`output/build-metrics/preview-calculation-2026-10-01T11-35-01.832Z-764220.json`,
SHA `2f871ceb472824e35a2ef34b4d987f40d73fd90b` with a dirty workspace marker.
All 18 workloads completed 20 warmups and 100 observations, with existing caches.
Frontend checks and native helper preparation were complete before this final
run. The original run had other VM activity, so these observations still do not
establish universal speedup percentages.

| Final scenario / clips | First calculation | p50 / p95 calculation | Sampled peak heap |
| --- | --- | --- | --- |
| Plain / 5,000 | 4.49 ms | 1.50 / 4.30 ms | 67 MiB |
| Crossfades / 1,000 | 5.15 ms | 1.58 / 2.18 ms | 51 MiB |
| Crossfades / 5,000 | 26.04 ms | 8.06 / 12.34 ms | 117 MiB |
| Nested crossfades / 100 | 3.54 ms | 0.15 / 0.48 ms | 44 MiB |
| Nested crossfades / 1,000 | 10.16 ms | 1.76 / 2.58 ms | 67 MiB |
| Nested crossfades / 5,000 | 61.23 ms | 8.27 / 9.82 ms | 156 MiB |
| Captions / 5,000 | 8.78 ms | 2.31 / 7.38 ms | 115 MiB |
| Many media / 5,000 | 4.18 ms | 2.61 / 5.59 ms | 122 MiB |
| Preparation-required effects / 5,000 | 68.88 ms | 10.84 / 13.23 ms | 128 MiB |

The previously incomplete crossfade/nested workloads now complete. Reused planning,
linear transition identity indexes and static nested-channel sampling address the
measured expensive paths. Plain/caption/many-media observations are slower in this
run; they avoid signature caching and do not support a broad performance claim.
First calculations are reported separately from warm spans. Animated nested
channels retain their original 120-Hz sampling, with oversize plans uncached.

Final parity evidence
`output/build-metrics/preview-parity-2026-10-01T11-29-47.616Z-758769.json`
compares 700 frames exactly against the original source snapshot: six deterministic
100-clip scenarios and an animated nested sweep. Targeted tests also exercise
in-place mutations, signed zero, unrepresentable extra properties, cache eviction
and both retention bounds. These checks cover calculation parity, not decoding.

## Actual retained project history

The real local project-history slice was measured with the same deterministic
plain/caption/many-media matrix and 125 edits, undo and redo. Before-budget evidence
is `output/build-metrics/history-memory-2026-10-01T11-23-16.944Z-742083.json`;
the existing 100-snapshot policy plateaued but retained substantial large histories.
The UTF-16 logical budget is a representation policy, not an exact heap estimate.

| 5,000-clip workload after 125 edits | Before retained snapshots / heap | After retained snapshots / heap | After UTF-8 snapshot representation |
| --- | --- | --- | --- |
| Plain | 100 / 210.7 MiB | 35 / 102.3 MiB | 62.4 MiB |
| Captions | 100 / 475.9 MiB | 14 / 108.1 MiB | 61.0 MiB |
| Many media | 100 / 319.2 MiB | 25 / 114.7 MiB | 62.9 MiB |

After-budget evidence is
`output/build-metrics/history-memory-2026-10-01T11-33-26.972Z-763189.json`.
All nine workloads completed; undo/redo moved the available nearest snapshots
exactly, with similar retained heap through those operations. Released heaps were
about 41–43 MiB. ASCII fixture representation doubles from UTF-8 bytes to UTF-16
logical bytes; measured retained logical totals are 124.9, 122.1 and 125.7 MiB.
The history budget materially reduces this measured retention while
reducing undo depth for large projects. It does not cap the complete application
heap, and the explicit one-oversize-snapshot exception can exceed the logical cap.
