# Linux MP4 presentation timing and sync-sample proof

- Date: 2026-09-12
- Host: Ubuntu 24.04.4 LTS, Linux 6.8.0-139-generic, x86_64
- Candidate: [`mp4` 0.14.0](https://crates.io/crates/mp4/0.14.0) from `alfg/mp4-rust`
- Source tag/commit: [`0.14.0`](https://github.com/alfg/mp4-rust/tree/0.14.0), `18f1718b70955e2982f81b9827aa396b533c4fbc`
- Verdict: **the released crate resolves minimp4's two specific progressive-MP4 table gaps, but the container gate remains partial**

## Result

The released `mp4` crate exposes decode start time, duration, signed composition
offset and sync status for progressive MP4 samples. A test-only adapter derived
presentation time and byte location from the public sample tables, then selected
the preceding sync sample and returned the decode-order prefix required to reach
the requested presentation sample. Static-musl release tests passed with a
standards-valid version-1 `ctts` fixture containing negative offsets.

This is a qualified improvement over the tested minimp4 source: `ctts` is retained
and `stss` is used. It is not a production selection. The release does not expose
sample byte offsets directly, does not apply edit lists, mishandles parts of the
signed/unsigned edit and composition-time formats, has inadequate fragmented-MP4
sample semantics, and can panic on inconsistent sample tables during
`read_sample`. The proof adapter rejects edit lists and fragmented files rather
than returning misleading timestamps.

No production pipeline file or dependency was changed. The adapter, source clone,
fixtures, lockfile, full license texts, binaries and raw logs are ignored under
`output/linux-mp4-timing-proof/`. They are evidence retained in this worktree, not
artifacts reproducible from the Git commit alone.

## Exact source and API evidence

The crates.io archive has SHA-256
`c9ef834d5ed55e494a2ae350220314dc4aacd1c43a9498b00e320e0ea352a5c3`.
Its packaged `.cargo_vcs_info.json` identifies commit
`18f1718b70955e2982f81b9827aa396b533c4fbc`, the same commit reached by the
upstream `0.14.0` tag. The upstream [manifest](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/Cargo.toml#L1-L25)
declares version 0.14.0, the six direct dependencies used here, and MIT; the full
[MIT license](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/LICENSE)
was retained and hashes to
`fbd5bc4849335a9e45100d9c71cad4d44e0cdeeb6a16556e6c3e988ac09ddbec`.

The public [`Mp4Sample`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/types.rs#L618-L625)
contains `start_time: u64`, `duration: u32`, `rendering_offset: i32`, `is_sync`
and bytes. The progressive reader derives byte offsets through `stsc` plus
`stco`/`co64`, derives decode time from `stts`, reads a rendering offset from
`ctts`, and checks sample IDs in `stss` before constructing that value:
[`track.rs`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/track.rs#L437-L563).
The proof therefore interprets `start_time` as decode time and computes
`PTS = checked_i64(DTS) + i64(composition_offset)`.

The direct API omits the byte offset. The local adapter independently expands
the public progressive sample tables, validates cross-table counts and checked
arithmetic, limits an index to 1,000,000 samples, and records offset and length.
The synthetic expected offsets were derived by independently locating each
unique payload in the file bytes, so the assertion does not reuse the adapter's
offset algorithm.

### Source limitations that affect the result

- `ctts` stores an `i32` and the reader always calls `read_i32`, regardless of
  box version: [`ctts.rs`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/mp4box/ctts.rs#L52-L88).
  Version 1 negative offsets work. Version 0 is unsigned in ISO BMFF, so offsets
  above `i32::MAX` are represented incorrectly. The writer also starts from a
  default version-0 box while accepting `i32` offsets:
  [`track.rs`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/track.rs#L688-L718).
  The fixture generator's output was patched from version 0 to version 1 before
  parsing; the encoded offset bits were unchanged.
- `stss` entries are read, but sample lookup assumes that they are sorted and
  uses `binary_search`; malformed ordering is not validated:
  [`track.rs`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/track.rs#L520-L530).
- `edts/elst` is parsed into the track, but no sample-time path applies it. More
  seriously, `ElstEntry.media_time` is `u64` even though the source comments and
  format read describe signed fields; both version branches read unsigned values:
  [`elst.rs`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/mp4box/elst.rs#L17-L23),
  [`elst.rs` read path](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/mp4box/elst.rs#L60-L109).
- Fragment tracks are populated by the reader, but their sample path returns one
  `tfhd.base_data_offset` for each sample, synthesizes DTS solely from one default
  duration, and invents sync points from fragment boundaries instead of sample
  flags: [`track.rs`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/track.rs#L368-L443),
  [`track.rs` timing/sync](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/track.rs#L475-L530).
  Fragmented MP4 is outside this pass.
- `read_sample` unwraps the `stts` result after allocating and reading sample
  bytes. An `stsz`/`stts` mismatch therefore panics rather than returning a typed
  error: [`track.rs`](https://github.com/alfg/mp4-rust/blob/18f1718b70955e2982f81b9827aa396b533c4fbc/src/track.rs#L533-L563).

## Locked dependency and license review

`Cargo.lock` hashes to
`42f344ed6baa5b35a5bb050d67ad589cf452bffdf00ebeade3fd253032934249`.
`cargo tree --locked --target x86_64-unknown-linux-musl -e normal` resolved the
following complete normal graph. Full license files from every registry package
were copied under `evidence/licenses/` before the build.

| Locked package | Declared expression | Review |
| --- | --- | --- |
| `mp4 0.14.0` | MIT | Permissive; full upstream text reviewed |
| `byteorder 1.5.0`, `memchr 2.8.3` | Unlicense OR MIT | Select MIT; full MIT texts retained |
| `bytes 1.12.1` | MIT | Permissive |
| `num-bigint 0.4.8`, `num-integer 0.1.47`, `num-rational 0.4.2`, `num-traits 0.2.19` | MIT OR Apache-2.0 | Permissive alternatives |
| `serde 1.0.229`, `serde_core 1.0.229`, `serde_derive 1.0.229`, `serde_json 1.0.151` | MIT OR Apache-2.0 | Permissive alternatives |
| `thiserror 1.0.69`, `thiserror-impl 1.0.69` | MIT OR Apache-2.0 | Permissive alternatives |
| `itoa 1.0.18`, `proc-macro2 1.0.107`, `quote 1.0.47`, `syn 2.0.119`, `syn 3.0.5` | MIT OR Apache-2.0 | Permissive alternatives |
| `autocfg 1.5.1` | Apache-2.0 OR MIT | Permissive alternatives |
| `unicode-ident 1.0.24` | (MIT OR Apache-2.0) AND Unicode-3.0 | Both required terms are permissive; full Unicode-3.0 text reviewed. `Unicode-3.0` still needs an explicit project allowlist entry because it is outside the initial six-family evaluator list. |
| `zmij 1.0.23` | MIT | Permissive |

There are no native library dependencies in this crate graph. Proc-macro and
build-script dependencies execute on the build host; the final static test
artifact has no `NEEDED` entries or unresolved symbols. Source-term review does
not establish final Rust toolchain/CRT provenance, notice packaging, SBOM
eligibility, or codec patent clearance. No `inventory-eligible` status is claimed.

## Synthetic timing and seeking result

The generated 876-byte progressive MP4 uses a 1,000-unit timescale, version-1
`ctts`, three `stts` durations, and two `stss` samples. The payloads are unique
ASCII marker bytes, not H.264 access units. This is a container-only test and
does not establish decoder acceptance, reference dependencies, frame content,
or codec conformance.

| ID | DTS | signed CTS offset | PTS | duration | byte offset | length | sync |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| 1 | 0 | 0 | 0 | 1000 | 40 | 23 | yes |
| 2 | 1000 | 2000 | 3000 | 1000 | 63 | 23 | no |
| 3 | 2000 | -1000 | 1000 | 1000 | 86 | 23 | no |
| 4 | 3000 | -1000 | 2000 | 1000 | 109 | 23 | no |
| 5 | 4000 | 0 | 4000 | 500 | 132 | 23 | yes |
| 6 | 4500 | 0 | 4500 | 1500 | 155 | 23 | no |

The fixture SHA-256 is
`2c48003bc436b73af1b4d326c42ed612894259632d42fcd67b920e2258aceae5`.
Different durations cover progressive VFR table expansion. The negative offsets
and non-monotonic PTS cover table-level presentation reordering only.

The seek adapter chooses the first presentation sample at or after the request,
or the last presentation sample after end of stream. It then walks backward in
decode order to the latest sync sample and returns every decode sample through
the target. Measured cases were:

| requested PTS | target sample | preceding sync | decode-forward IDs |
| ---: | ---: | ---: | --- |
| 0 | 1 | 1 | `1` |
| 1000 | 3 | 1 | `1,2,3` |
| 2000 | 4 | 1 | `1,2,3,4` |
| 2999 | 2 | 1 | `1,2` |
| 4000 | 5 | 5 | `5` |
| 4400 | 6 | 5 | `5,6` |
| 9000 | 6 | 5 | `5,6` |

This establishes container selection and decode ordering. It does not prove that
decoding only through the target decode sample is sufficient to emit every frame
near a requested PTS; a real codec may buffer and reorder output.

## Provenance fixture result

The existing
`src-tauri/tests/fixtures/media/edison-speech-1920s-30s.mp4` was copied into the
ignored proof unchanged. Its adjacent provenance document identifies the public
domain U.S. National Archives/Wikimedia source, exact reproduction command, and
SHA-256 `05b0da3bc3f54f9a278f5890e38dd49f74c835e58c829d24f227e4f676d0b5a2`.
This is the repository's valid H.264/AAC fixture; the shorter sample-project
`media/input.mp4` has its AAC in a separate `voiceover.m4a` and was correctly
excluded after the first integration assertion exposed that distinction.

The released crate parsed and read all nonempty access units:

- H.264 track 1: 899 samples, five sync samples, composition offsets 0 through
  5,005; first sample DTS 0, offset 2,002, sync true.
- AAC track 2: 1,293 samples; first sample DTS 0 and composition offset 0.
- Parsed edit lists were video `(segment_duration=29997, media_time=2002,
  rate=1)` and audio `(30000, 1024, 1)`.

This confirms H.264/AAC sample-entry and access-unit reading against a valid,
provenanced container. No H.264 or AAC decoder ran in this task, so the result
does not independently validate codec bytes or edit-adjusted A/V alignment.

## Malformed and bounded behavior

Under a 120-second timeout and 4 GiB virtual-memory ceiling, the final release
run used 272,248 KiB maximum resident memory. All 876 strict truncations of the
synthetic file plus three malformed top-level box cases returned errors without
parser panics. The same release accepted a deliberately inconsistent file whose
`stsz` describes six samples while `stts` describes zero; `read_sample(1, 1)`
then panicked at `track.rs:553` on `EntryInStblNotFound`. The harness catches and
reports that panic, so the seven-test suite passes while the malformed sample-read
capability is explicitly failed.

The local adapter rejects cross-table count mismatches, invalid/unsorted `stss`,
invalid chunk mappings, arithmetic overflow, unsupported `ctts` versions,
version-0 offsets that overflow the crate's signed representation, edit lists,
fragmented files, and indexes above 1,000,000 samples. This is prototype evidence,
not an upstream repair or production parser hardening.

## Static-musl build evidence

The final command was:

```text
ulimit -v 4194304
CARGO_BUILD_JOBS=1 cargo test --locked --release \
  --target x86_64-unknown-linux-musl --all-targets -- \
  --nocapture --test-threads=1
```

Result: seven passed, zero failed in the timing integration binary. One expected
panic line appears because the malformed cross-table characterization catches
the released crate's panic. `/usr/bin/time -v` reported 17.94 seconds elapsed and
272,248 KiB maximum resident set size.

The test artifact
`target/x86_64-unknown-linux-musl/release/deps/timing-78a8c632d2e809f9`
hashes to
`2ccf1138d3c6e2a57f23556e8f3b893d92c2450e2cb1c47a681a89072da71db2`.
`file` identifies an x86-64 static PIE ELF. `readelf` reports no interpreter and
no `NEEDED` entry; `ldd` reports `statically linked`; `nm -u` prints no undefined
symbols.

Two superseded release attempts are retained rather than omitted. A 1 GiB
virtual-memory ceiling caused host lld to fail thread creation, and applying an
lld-only `--threads=1` argument globally then failed because the musl target used
GNU ld. Raising the bound to 4 GiB, keeping one Cargo job, and removing the
incompatible linker argument produced the passing result above. These are build
environment outcomes, not candidate-code failures.

Raw retained evidence includes the red/green adapter runs, full test output,
dependency tree, registry metadata, source pin, license texts, ELF headers,
hashes and both failed build transcripts.

## Coverage and open gates

| Area | Status | Evidence or gap |
| --- | --- | --- |
| Progressive `ctts` / signed offsets | Passed narrowly | Version-1 negative offsets asserted; version-0 values above `i32::MAX` remain incorrect |
| Progressive `stss` / preceding-sync selection | Passed narrowly | Multiple requests and two sync points asserted; malformed ordering rejected only by local adapter |
| DTS, PTS, duration, offset, length | Passed narrowly | Six table-derived samples; only `stco`-sized fixture offsets exercised, not large `co64` media |
| VFR | Passed at table level | Three sample durations; no codec playback or A/V sync |
| Provenanced H.264/AAC MP4 | Passed for container reads | Every access unit nonempty; no codec decode in this task |
| B-frame/reordered output | Open | Composition-order table proven; decoder buffering/output order and frame identity untested |
| Edit lists | Failed/open | Real entries parse, but are not applied; signed media-time representation is wrong; adapter rejects |
| Fragmented MP4 | Failed/open | Released sample offset/timing/sync logic is insufficient; adapter rejects |
| Rotation | Open | Track matrix parsing exists, but no orientation adapter or visual check ran |
| Large timestamps/offsets and `co64` | Open | Checked locally, not fixture-tested; crate's version-0 `ctts` range is inadequate |
| Malformed top-level/truncation | Passed narrowly | 879 bounded cases returned errors without panic |
| Malformed cross-table sample read | Failed | Reproducible released-crate panic, caught and reported |

## Combined codec/container proof still required

The candidate answers the two minimp4 failures only for qualified progressive
inputs. A combined worker must still:

1. Apply movie/track edit lists and define presentation time in the canonical
   project timebase, including empty edits and signed media times.
2. Seek to an actual preceding H.264 random-access sample, inject the required
   `avcC` SPS/PPS, convert AVCC access units for the selected decoder, decode
   forward, flush delayed output, reorder by PTS, and verify requested frame
   identity within a stated tolerance.
3. Extract AAC access units plus AudioSpecificConfig, decode PCM, account for
   priming/delay and edits, and measure A/V synchronization after seeks and cuts.
4. Exercise real B-frame H.264, VFR, rotation, `co64`/large values, malformed
   sample tables, and either support fragmented MP4 correctly or reject it as a
   documented input limitation subject to the common-MP4 product requirement.
5. Prove the reverse path: encode H.264/AAC, mux timestamps/keyframe flags and
   configuration, finalize MP4, then reopen and validate streams, duration,
   frame content and A/V sync with an independent implementation.
6. Complete artifact/toolchain provenance, notices, SBOM, performance and format
   corpus review. The separate codec-runtime result must qualify the OpenH264 and
   libxaac runtime closure before any complete backend decision.

The appropriate disposition is to retain `mp4` 0.14.0 as a technically useful
progressive-container candidate, not approve it as the production container
backend. Its `ctts` and `stss` behavior removes the exact minimp4 blockers, while
edit semantics, fragmented files and malformed-table robustness remain real
gates rather than reduced scope.
