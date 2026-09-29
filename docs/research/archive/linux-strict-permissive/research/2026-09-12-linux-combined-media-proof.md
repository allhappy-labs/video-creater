# Linux combined owned-media proof

**combined owned-fixture proof passed; production media backend remains unqualified**

- Date: 2026-09-12
- Target: `x86_64-unknown-linux-musl`
- Owned fixture: 64x48 I420 at 30 fps for one second; stereo PCM16 at 48 kHz
- Candidate codecs/container: OpenH264 2.6.0, libxaac 0.1.13, and `mp4` 0.14.0
- Current evidence: `output/linux-combined-media-proof/evidence-attempt13/`

## Result and boundary

Attempt 13 records a passing one-process media round trip under the current
verifier. The executable generated 30 deterministic I420 frames and 48,000
stereo PCM sample frames, encoded H.264 and AAC-LC through direct in-process
codec APIs, wrote a progressive MP4 with `mp4` 0.14.0, reopened and demuxed that
file with the same crate, reconstructed Annex-B access units from MP4 AVCC
samples, decoded both streams with the same codec builds, and compared the
decoded output with the owned inputs. `raw/runtime.stdout` contains exactly
`COMBINED_MEDIA_PASS`.

This is integration evidence for one fixed profile. OpenH264 performs both the
H.264 encode and decode, libxaac performs both the AAC encode and decode, and
`mp4` 0.14.0 performs both the mux and demux. No independent decoder or
container implementation checked the resulting bytes. The audio comparison is
limited to a central 36,000-frame window. It does not inspect complete-tail
recovery or establish gapless behavior or general A/V synchronization.

The generated `combined.mp4` is 52,095 bytes and hashes to
`4ffa7e777632cd40cb908aa0a530f6ec1bc8b7fb74723c9f832b1baff55ff465`.
The verifier calculated this digest after execution with Node's built-in
`node:crypto`; it did not delegate media hashing to a runtime helper. The file
contains exactly one `vide` track and one `soun` track.

## Evidence identity

| Evidence item | SHA-256 | Meaning |
| --- | --- | --- |
| Frozen prepared base, `inputs/prepared-inputs.json` | `ed1be347b5d8155ca5d41002973db948f6a9ea574352473a5a537d331f86e170` | Fully prepared Task 1 source/toolchain snapshot |
| Accepted Cargo-only derivation, `inputs/prepared-inputs-task3.json` | `db29d85fd2957f1f1ea1f0e77a6f322619d001e4e029db366a14b224f3c9ead9` | Same frozen source/toolchain/native inventory with the reviewed root `serde_json` build edge |
| Native attempt-4 manifest | `ce89fb8f9fe9bc5584846db14cf7d294c83983242157eebf5af9efef335f263e` | Accepted native codec/runtime build inputs and commands |
| Final build attempt-2 receipt | `6111941c0187f5ee9c070d14080dfe86176efc7c14827588c07652d1b31cbcd7` | Accepted Cargo build and receipt bundle; attempt 13 enforces singular rustc output identity |
| Cargo-selected executable and receipt-selected link output | `ae24339fa592624d2f3ea4fb4486bed8601a48beadd39762e1cabdcbfa9a0d7a` | Equal byte hashes for the final executable and actual link output |
| Source pre/post/current inventory identity | `2e7ba692f53908d0e9140bdbd012aea6e5c4b0d498ce712c353544980195edb7` | Stable source-byte inventory recorded by the build evidence |
| Selected LLD map | `886ff7f5f7902ba0e2fd4f0d8ce0c4c6d489e82c44b149bdc95a972ae57b33d2` | Map used for all final-contributor classifications |
| Attempt-13 `summary.json` | `0b697df7b55f1e65b54e9fece746022d479a33d841b6619c4be88edb63f0bea9` | Accepted passing verifier summary; Fresh Evidence Gate PASS |
| Attempt-13 `artifact-hashes.json` | `6d3cbea7e262d91cc743a032dc111c1f0f5e453db05c20c063f9e0c6e0cc84b2` | 265 hash records; 264 total files comprise 262 eligible files plus the self-excluded manifest and summary, with zero mismatches |
| Attempt-13 `provenance/verifier-identity.json` | `cf25f318d8e4c1c6a2e3467195f2d9f70bfa4ed2aff9d9eb8ad141bab00c0194` | Stable verifier source identity, canonical invocation, Node identity, and imported helper identities |
| Attempt-13 `provenance/contributors.json` | `cc056a7acb1b9e9e9a91f57ad3bce4ca2c237b2b8150368375313edcb1a162d5` | The unchanged 596 parsed and classified selected map rows |

Failed build `final-build` and failed verifier roots `evidence-attempt1`,
`evidence-attempt2`, `evidence-attempt3`, and `evidence-attempt5` remain
preserved and are not passing evidence. Cargo completed in failed build attempt
1, but final receipt validation rejected two ephemeral LLVM-IR aliases that had
already disappeared. The accepted attempt-2 driver records 27 host-rustc
receipts, 19 target-rustc receipts, 16 retained `.rcgu.o` bindings, and a
receipt-bound bridge archive. Evidence attempts 4 and 6 passed older verifier
states. Attempt 7 passed its implemented checks but was incomplete under Fresh
Evidence Gate review. Attempt 8 passed the first focused review fix, then was
superseded after final diff review closed the remaining exact link-argv gap.
Attempt 9 passed an incomplete receipt-inventory check and is superseded.
Attempt 10 passed a receipt inventory that could not distinguish the two
`serde_core` custom-build variants and is review-incomplete. Attempt 11 added
logical variant identity but checked compile metadata against argv only for
target receipts. Attempt 12 made that check unconditional but accepted a later
conflicting identity option. Both are review-incomplete. Attempt 13 binds the
committed `b1a50de` verifier bytes and supersedes attempts 7 through 12.
Attempt 1 led to exact normal-exit trace handling,
attempt 2 exposed `getrandom`, attempt 3 showed that LLD did not emit standalone
undefined-name rows, and attempt 5 stopped during the stricter archive-member
classification work.

Attempt 13 authenticates the pinned Cargo executable, exact Cargo argv,
repository cwd, complete environment, raw Cargo message inventory, and both
recording launchers. It follows the strict chain through all 46 rustc receipts
(27 host and 19 target), including target, crate, source input, output
directory, extra filename, emit/release/LTO flags, and output contracts; it
also checks all 16 retained object bindings, the bridge compiler/archiver
commands and member order, and the final-link static-PIE/response-file/output
contract. Focused negative tests coherently rewrite receipts and hashes while
changing one required semantic and require rejection.

The current verifier checks receipt completeness against an independent fixed
multiset for this Rust 1.97.1, 22-registry-package graph. It requires the 39
expected Cargo compiler-artifact records to match 39 observed events and pairs
them one-to-one with 39 compile rustc receipts by source, crate,
host-or-target classification, output directory, and artifact filenames: 23
host and 16 target. It separately requires seven exact special forms: host
`--version --verbose` once, host `-vV` twice, exact Cargo capability queries
once for host and target, and the target autocfg `stdin_0` and `stdin_1` probes
once each. The resulting 27 host plus 19 target equals 46 total is an
additional count assertion rather than the basis of completeness. Focused
tests reject coherent host/target receipt-and-event deletion, special deletion,
duplicates, unmatched receipts, and wrong query forms.

Fix `b0e0c97a` also binds exact Cargo features/profile and coherent
artifact/output identity. The two actual `serde_core` custom-build variants are
distinct—features `["result", "std"]` with output identity beginning `5253…`,
and features `["std"]` with output identity beginning `4f1d…`. Each binds to
the corresponding rustc `--cfg feature` values, codegen profile, extra
filename, output directory, artifact filenames, and emitted outputs. A focused
negative copies one survivor to a different JSON path and coherently mutates
the Cargo event, rustc receipt, and raw references; it is rejected as a
duplicate logical invocation. Count and path uniqueness alone are insufficient.

Fix `809ded3` enforces the metadata-to-argv bindings, along with pinned rustc
path, classification/target, source identity, emitted outputs, feature cfgs,
and codegen profile, unconditionally for all 23 host and 16 target compile
receipts. Its full host `serde_core` survivor-copy negative coherently mutates
the Cargo event, receipt, and raw references and is rejected.

Fix `b1a50de` requires exactly one logical occurrence of every output-identity
option across split and combined argv forms for host and target, including
`extra-filename`, and rejects duplicate or conflicting values. This remains a
fixed-graph check, not a general future Rust or dependency-graph validator. The
Fresh Evidence Gate reviewed and accepted the fix and attempt 13 with no
remaining findings or blocked checks inside the bounded Task 5b scope.

The verifier entry source hashes to
`f6abbfab00a1ef8cc9d1f7f1cdc9e3cd99d65c8249f29354c91c6e8e3dbc0a75`.
`provenance/verifier-identity.json` records `sourceStable: true`, the canonical
options argv, and Node v24.18.1 at its absolute path with SHA-256
`f3432a45b03b2da0d270095fdd8813dc34cbea73f5fc8b18c7a384b7cf9b333a`.
It also records imported build-validator hash
`6eecf8a71ce6c30a056be411db216d30d38f21345a30e586d22ffded121aa03b`
and preparation-validator hash
`7e148f6b7bf9deea69cc16a4837eee251bb039dfdd3ea6ea2d66455f2e6d7b94`.
All three source copies and the identity record are explicit artifact-manifest
entries, and the live sources match before and after the run. This is local
byte and invocation provenance, not signed execution attestation or caller
identity.

## Source and toolchain provenance

The checked-in lock itself hashes to
`c8bf30e8a8398bac5e4f48409ee492c49961208820fc25a67585bc1c42a456b7`.
The retained provenance copies are under
`evidence-attempt13/provenance/source-lock.json`, `source-revisions.json`,
`prepared-inputs.json`, and `native-build.json`.

| Input | Exact version, revision, or archive SHA-256 | Role and source term |
| --- | --- | --- |
| OpenH264 | commit `652bdb7719f30b52b08e506645a7322ff1b2cc6f`; `libopenh264.a` `5e2bf20844cdf787c67f7e629f9d8866a6bfae78085eaee93fd0ae2d719291a6` | Direct H.264 encoder/decoder; BSD-2-Clause |
| libxaac | commit `55af74f9ce7cadb99fbd7870c6487c8a12d75689`; `libxaacenc.a` `221cec84522eaf30b9722041521ea7383b6009b1011ed0b661ab6b1107199b0b`; `libxaacdec.a` `422c5c3a126efbac7f846c0a6a0607f318ef81570c5dc9c4d763891cf71a692c` | Direct AAC-LC encoder/decoder; Apache-2.0, with retained upstream NOTICE |
| LLVM project | commit `3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff`; `libc++abi.a` `5d55b55dd0aac18e2ad36d8a1d4e3f9f0d04a8f7867c8ef748041bd35c88bda9`; `libunwind.a` `2dc07227f9ddc50f99b046ddc493ef2e1f5f3bf0ac8a4d13cfe445739b5c4433`; `crtbeginS.o` `4a18df51efe689807a0855cb437a6f5accf6d8b0f5911770ec6c7de0eb6487b4`; `crtendS.o` `3763ada3d474034f5bc0b756f6ccd17d3586accd87ad3b62b73ed152bad064d2` | C++ ABI, unwind, and compiler CRT; `Apache-2.0 WITH LLVM-exception` |
| Rust channel | 1.97.1 manifest `03569b1886ceb5c05276b50c8431ab111de944cd6140fe1fa7d821dd8e0f29cf`; rustc commit `8bab26f4f68e0e26f0bb7960be334d5b520ea452` | Pinned private Rust/Cargo toolchain; MIT OR Apache-2.0 source-level snapshot |
| Rust host archives | rustc `9819d0a32d56bd339585319c80260e332779f5541fd66838ab7e016d6c814819`; Cargo `e1be5f5ff7f7f80ca506fb65770b759edbdc6d303781ed71c5de8ec8a8394779`; host std `1c1e704ae80126b7de34f72ea2825f7fd01736dec20732faed47374b95282fba` | Development compiler, package manager, and host standard library |
| Rust musl standard library | official archive `51d83178680556f73a5fa8ad865b76a1ff541867445c00fc65dc67246bc2de66`; installed contributing `libstd` `8ff60d366af7bc26c8808031a1384bb3a681ee9a65f44cb35a655e7e4cfe143c` | Exact installed rlibs were bound to members of the verified official target archive |
| musl | Ubuntu 1.2.4-2 archives: runtime `9f0883c20b4b746e05e947bafd99cb933f5494ffaaa6fcd360cbe1fbcf264883`, development `4b451ecb6a0f8469883058cf22a807f3bd9cc16d115cc08b7efc35fe8eb44db2`, retained-only tools `46c01d212d3eb3a1322693089037f0a5c92383a089d39c392db3c86c19ffb229`; selected `libc.a` `657e951f4c1d02ecac4e424222034c84a189f361b090f1284f255b3ac4480d5c` | C/POSIX runtime and CRT; MIT source-level snapshot |
| musl CRT | `rcrt1.o` `e5867f7fccfa94384406ed3bf21a8db8756621f99d464c2c624de9f817291d7d`; `crti.o` `d92d0ca1f8ae3c6fa11ae848e93da11dac9f090981bb831d60c86ae99a9aff13`; `crtn.o` `6152eb43c1a39390a105eb459701e2606c4f7c3c4b17b8b2cad83cb690cb5670` | Static-PIE startup and init/fini framing from the pinned musl package |
| `mp4` | version 0.14.0, upstream commit `18f1718b70955e2982f81b9827aa396b533c4fbc`, crate archive `c9ef834d5ed55e494a2ae350220314dc4aacd1c43a9498b00e320e0ea352a5c3` | Progressive MP4 mux/demux; MIT |
| Clang/LLD packages | Clang `628b16701014ef7ad648380b20ea74b90dd543857f933b3e34d2fc042783de25`; headers `d41631bb752815a8196261d1030c096591c59678c8adfbf2e6a740a691b49e80`; LLD `c574594d714bf445f4c2323fa99374c05b79be09a3899edbf9d4b7265c9ccc57`; linker tools `50f0bcfa784e42f8bf0eac31aeb6a20798fb6174cc31efe021f12ae71af91b23` | Private development-only compiler/linker packages; no target runtime role |

The official Rust target-archive membership closes the earlier distribution
origin gap for these exact retained Rust rlibs only. It does not prove a
source-reproducible standard library, identify every source file that produced
each member, or complete a per-file license and redistribution-obligation
audit.

Ten checked-in source-level notice snapshots were byte-compared with their
locked OpenH264, libxaac, LLVM libc++/libc++abi/libunwind/compiler-rt, musl,
Rust, and `mp4` sources. The copies and mapping are in
`native/linux-combined-media-proof/notices/` and the attempt-13
`provenance/notices/` tree. These notices support source-term review; they do
not establish that every per-file, third-party, attribution, patent, or
distribution obligation for a shipped binary has been identified or met.

## Locked Cargo graph and licenses

`native/linux-combined-media-proof/Cargo.lock` hashes to
`fe666c3b994c4287553717924827cc9646dd1b1c0128d85df5e595e956c4c28c`.
The normal target graph below is the actual output of pinned Cargo 1.97.1 with
`--locked --offline --target x86_64-unknown-linux-musl -e normal`; it contains
21 third-party packages. Exact crate checksums and retained full license texts
are recorded in `evidence-attempt13/provenance/cargo-metadata.json` and
`cargo-licenses/`.

| Normal dependency packages | Locked versions | Declared expression |
| --- | --- | --- |
| `mp4`, `bytes`, `itoa`, `zmij` | 0.14.0; 1.12.1; 1.0.18; 1.0.23 | MIT |
| `byteorder`, `memchr` | 1.5.0; 2.8.3 | Unlicense OR MIT |
| `num-bigint`, `num-integer`, `num-rational`, `num-traits` | 0.4.8; 0.1.47; 0.4.2; 0.2.19 | MIT OR Apache-2.0 |
| `serde`, `serde_core`, `serde_derive`, `serde_json` | 1.0.229; 1.0.229; 1.0.229; 1.0.151 | MIT OR Apache-2.0 |
| `proc-macro2`, `quote`, `syn`, `syn` | 1.0.107; 1.0.47; 2.0.119; 3.0.5 | MIT OR Apache-2.0 |
| `thiserror`, `thiserror-impl` | 1.0.69; 1.0.69 | MIT OR Apache-2.0 |
| `unicode-ident` | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |

The root crate also declares `serde_json` 1.0.151 as a build dependency so
`build.rs` can parse the native manifest. That host-executed edge is
development-only. The same locked `serde_json` package is independently in the
normal target graph through `mp4`, so it must not be described as wholly
build-only. `autocfg` 1.5.1 (`Apache-2.0 OR MIT`) is a build dependency of
`num-traits` and is outside the normal graph. Proc-macro crates execute on the
build host even though Cargo displays them along the normal dependency path;
final link-map classification, rather than Cargo edge type, determines which
bytes contributed to the executable.

## Final contributor classification

The selected LLD map contains 596 contributor rows. The attempt-13 verifier
classified every row and recorded zero forbidden or unclassified rows.

| Classification | Rows | Evidence boundary |
| --- | ---: | --- |
| Generated aggregate native member | 364 | Members of retained rustc aggregate `3b83afab9a5cc2d3dd291988b0c5e67fdad82d91ec6774488ed8f467d189cfb3`, qualified through approved native archives and bridge receipts |
| Native archive member | 225 | Selected members of pinned musl `libc.a` `657e951f4c1d02ecac4e424222034c84a189f361b090f1284f255b3ac4480d5c` |
| Native object | 4 | `crtbeginS.o`, `rcrt1.o`, `crti.o`, and `crtn.o` at the hashes above |
| Rust-generated object | 1 | Retained receipt-bound `.rcgu.o` `4f19f742e525f88fcfb593330b02a084c40fc99e946d4203a5367458b33e3dbd` |
| Official Rust archive member | 1 | One uniquely hashed `compiler_builtins` member from rlib `8a089f03dea685c0ad73d561540406f2ea0729788c162d76d17ea1832804a095` and the verified Rust target archive |
| Linker internal | 1 | LLD's `<internal>` row |
| **Total** | **596** | **Forbidden 0; unclassified 0** |

The generated aggregate is not a same-hash alias of the canonical crate rlib:
the canonical rlib hashes to
`7e579b51e00ef38991d8ecdb674a7a92e938eee1f192aa7385c4ced338e62eec`.
The retained target-rustc invocation, exact final-link argv consumption, and
per-member candidate provenance qualify the generated aggregate while
preserving its byte-hash distinction from the canonical rlib. There is no
separate receipt for rustc's internal aggregate-archive creation, and none is
claimed.

The 364 aggregate rows divide by qualified candidate origin as follows:

| Candidate origin set | Contributing rows |
| --- | ---: |
| `libopenh264.a` | 82 |
| `libxaacenc.a` | 135 |
| `libxaacdec.a` | 122 |
| `libxaacenc.a` + `libxaacdec.a` | 4 |
| `libc++abi.a` | 14 |
| `libc++abi.a` + `libunwind.a` | 5 |
| `libvc_codec_bridge.a` | 2 |

The aggregate was assembled from the recorded Cargo build-script order
`[vc_codec_bridge, openh264, xaacenc, xaacdec, c++abi, unwind]`, with all
archives selected statically from the accepted native root. Thirteen
contributing aggregate member names have multiple candidates: eight candidate
sets have identical member hashes, while five have different bytes (four
OpenH264 names plus libxaac's cross-archive `impd_drc_api.c.o`). Three directly
selected musl names—`realloc.lo`, `free.lo`, and `clone.lo`—also have two
candidates in the same pinned `libc.a`. `provenance/contributors.json` retains
each complete candidate sequence, size, hash, multiplicity, parent archive,
source identity, revision, and license origin.

LLD's `archive(member)` map notation does not identify which duplicate
occurrence supplied the selected section. The candidate origins are qualified,
but the selected occurrence and its exact member hash remain ambiguous. This
report does not choose a first match or claim that any one candidate hash was
the selected occurrence.

## Codec and container path

The C++ bridge calls `WelsCreateSVCEncoder`, `InitializeExt`, and `EncodeFrame`
directly. It removes Annex-B start codes, stores length-prefixed AVCC samples,
and retains SPS/PPS in the MP4 `avcC` record. On reopen, it prepends SPS/PPS to
the first access unit, converts every AVCC NAL back to Annex-B, calls
`WelsCreateDecoder` and `DecodeFrameNoDelay`, and drains buffered frames.

The C bridge calls `ixheaace_create` and `ixheaace_process` directly with raw
AAC-LC output. The decoder uses `ixheaacd_dec_api` in MP4/raw-access-unit mode,
parses the two-byte LC/48-kHz/stereo AudioSpecificConfig, initializes with the
first access unit, decodes all access units, signals input-over, and drains the
peak-limiter buffer. Neither codec bridge invokes an upstream command-line
testbench.

The progressive guard accepts this exact two-track shape and validates `stts`,
`stsz`, zero `ctts` offsets, ordered/in-range `stss`, media-header duration,
sample counts, and checked arithmetic. It rejects fragments, edit lists,
nonzero composition offsets, wrong track counts/types, and inconsistent sample
tables. The file uses 30 video samples of 3,000 ticks at a 90,000 timescale and
47 AAC samples of 1,024 ticks at a 48,000 timescale. This guard is bounded to
the owned proof and does not supply the production common-MP4 contract.

## Video fidelity

All 30 decoded frames were compared in presentation order to the same-index
owned source frame.

| Check | Actual | Acceptance | Attempt-13 result |
| --- | ---: | ---: | --- |
| Decoded frame count | 30 | exactly 30 | PASS |
| Maximum Y mean absolute error | 1.539388 | <= 18.000000 | PASS |
| Maximum U mean absolute error | 0.973958 | <= 22.000000 | PASS |
| Maximum V mean absolute error | 0.688802 | <= 22.000000 | PASS |

These values are lossy-codec fidelity measurements for one 64x48,
constrained-baseline-compatible, no-B-frame fixture. They are not visual
quality, color-management, profile coverage, or H.264 conformance results.

## Audio recovery

The validator selected the highest normalized correlation over the overlapping
central 36,000 source frames for lags from -2,048 through +2,048, independently
for each channel. RMS used the same aligned window.

| Check | Left | Right | Acceptance | Attempt-13 result |
| --- | ---: | ---: | --- | --- |
| Decoded sample frames | 48,128 | 48,128 | 48,000 through 49,023 | PASS |
| Coded padding frames | 128 | 128 | <= 1,023 | PASS |
| Selected lag, sample frames | 1,600 | 1,600 | absolute value <= 2,048 | PASS |
| Channel lag delta | \- | 0 | <= 1 frame | PASS |
| Normalized correlation | 0.99993968 | 0.99994215 | >= 0.90000000 each | PASS |
| RMS ratio | 1.000005 | 0.999756 | 0.700000 through 1.300000 each | PASS |

The 1,600-frame lag is about 33.33 ms at 48 kHz. It is recovered content
alignment inside the central comparison window. The 128-frame padding value is
the difference between 48,128 decoded/coded frames and the 48,000-frame input.
Neither number is an encoder-delay edit, a gapless declaration, or a full-tail
comparison.

## MP4 track timing

| Track/check | Actual | Acceptance | Attempt-13 result |
| --- | ---: | ---: | --- |
| Video track count | 1 | exactly 1 | PASS |
| Audio track count | 1 | exactly 1 | PASS |
| Video duration | 1,000,000 us | exactly 1,000,000 us | PASS |
| AAC sample duration | 1,024 at 48,000 Hz | exactly 1,024 | PASS |
| Coded audio duration | 1,002,666 us | reported | measured |
| Absolute coded-audio/video end delta | 2,666 us | <= 21,334 us | PASS |

The coded end delta describes this MP4's sample-table duration. It is separate
from the 1,600-frame decoded-content lag and does not prove production sync,
cut-boundary sync, seek behavior, playback-clock behavior, or gapless output.

## ELF and runtime-process evidence

| Check | Attempt-13 evidence | Result |
| --- | --- | --- |
| Binary format | ELF64, x86-64, type `DYN`, static PIE | PASS |
| Interpreter | no `INTERP` program header | PASS |
| Dynamic libraries | no `DT_NEEDED`; dynamic string table size 1 | PASS |
| Strong unresolved symbols | none | PASS |
| Process trace | one initial proof `execve`, then `exit_group(0)` | PASS |
| Helper/subprocess execution | none observed | PASS |
| Thread creation | zero `clone`/`clone3` thread calls observed | PASS |
| Runtime environment | `PATH=`; `LC_ALL=C`; `TZ=UTC` | recorded |
| Runtime-created files | `combined.mp4`, `metrics.txt` | bounded |

Raw proof is in `evidence-attempt13/raw/file.stdout`,
`readelf-header.stdout`, `readelf-program-headers.stdout`,
`readelf-dynamic.stdout`, `readelf-dynamic-symbols.stdout`,
`readelf-relocations.stdout`, `nm-undefined.stdout`, `process.trace`,
`runtime.stdout`, and `runtime.stderr`. The complete trace has only the initial
`execve` and normal exit. The OpenH264 bitrate/frame-skip diagnostic remains in
`runtime.stderr`; the proof still exited zero and emitted the fixed success
marker.

The five weak undefined names are `__cxa_finalize`,
`__deregister_frame_info`, `__register_frame_info`, `getrandom`, and `gettid`.
The first three bind to selected `crtbeginS.o`; `gettid` binds to the retained
receipt-bound `.rcgu.o`. `__cxa_thread_atexit_impl` is absent and is not claimed.

`getrandom` is permitted only for executable
`ae24339fa592624d2f3ea4fb4486bed8601a48beadd39762e1cabdcbfa9a0d7a`
when every predicate recorded in
[`2026-09-12-linux-getrandom-weak-probe.md`](./2026-09-12-linux-getrandom-weak-probe.md)
holds together: exact Rust commit and official Rust/libstd/musl hashes; weak
undefined symbol status; absence from `.dynsym`, dynamic relocations,
`DT_NEEDED`, and loaded-library contributors; selected musl `syscall.lo` and
excluded `getrandom.lo`; no interpreter; no dynamic-loader API references; and
all ordinary receipt, contributor, runtime, functional, and evidence checks.
The independent audit at `ea28d77d` and file-offset correction at `792df15b`
were reviewed and accepted. This creates no general `getrandom` allowlist and
no allowance for `pidfd_getpid`, `pidfd_spawnp`, or another unresolved symbol.

The final build set `CARGO_BUILD_JOBS=2`, which records a Cargo scheduling cap.
It is not a measurement of all compiler or linker threads. Runtime tracing
separately observed zero thread-creation calls in the proof execution. The
inner retained `provenance/build-evidence/logs/cargo.stderr` has 40 emitted
jobserver-FD warning records and hashes to
`6feecbb383bd9381fdbac45cb9c53307280c97db3714fc4b3098201e8d135216`;
the build is not warning-free.

## Exact commands and retained logs

The fresh base inputs and accepted native attempt were produced with:

```bash
rtk node scripts/prepare-linux-combined-media-proof.mjs \
  --lock "$PWD/native/linux-combined-media-proof/source-lock.json" \
  --manifest "$PWD/native/linux-combined-media-proof/Cargo.toml" \
  --output "$PWD/output/linux-combined-media-proof/inputs"

rtk node scripts/build-linux-combined-media-codecs.mjs \
  --prepared "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs.json" \
  --output "$PWD/output/linux-combined-media-proof/native-build-attempt4"
```

The current final binary and attempt-13 evidence were produced with caller-owned
new roots:

```bash
rtk node scripts/build-linux-combined-media-proof.mjs \
  --prepared "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs-task3.json" \
  --native-build "$PWD/output/linux-combined-media-proof/native-build-attempt4/native-build.json" \
  --output "$PWD/output/linux-combined-media-proof/final-build-attempt2"

rtk readlink -f /usr/bin/nm
rtk readlink -f /usr/bin/readelf

rtk node scripts/verify-linux-combined-media-proof.mjs \
  --build-receipt "$PWD/output/linux-combined-media-proof/final-build-attempt2/build-receipt.json" \
  --prepared "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs-task3.json" \
  --native-build "$PWD/output/linux-combined-media-proof/native-build-attempt4/native-build.json" \
  --output "$PWD/output/linux-combined-media-proof/evidence-attempt13" \
  --readelf /usr/bin/x86_64-linux-gnu-readelf \
  --nm /usr/bin/x86_64-linux-gnu-nm \
  --file /usr/bin/file \
  --strace /usr/bin/strace
```

Attempt 13's runtime command record is equivalent to:

```text
/usr/bin/strace -f -qq -v -s 4096 -e trace=process -e signal=none \
  <receipt-selected-linux-combined-media-proof> <new-runtime-output>
```

with `PATH` empty, `LC_ALL=C`, and `TZ=UTC`. Exact resolved commands,
arguments, environments, timestamps, exits, and stdout/stderr hashes are in
`provenance/verifier-commands.json`, `provenance/build-receipt.json`,
`provenance/build-evidence/receipts/`, and `provenance/native-commands/`.

The focused Task 5b test commands were:

```bash
rtk node --test \
  scripts/build-linux-combined-media-proof.test.ts \
  scripts/verify-linux-combined-media-proof.test.ts

rtk env \
  RUSTC="$PWD/output/linux-combined-media-proof/inputs/rust/bin/rustc" \
  CARGO_HOME="$PWD/output/linux-combined-media-proof/inputs/cargo-home" \
  "$PWD/output/linux-combined-media-proof/inputs/rust/bin/cargo" test \
  --manifest-path native/linux-combined-media-proof/Cargo.toml \
  --locked --offline --no-default-features cli_
```

Those focused runs passed 206 Node tests (144 verifier and 62 final-build
driver tests) and four CLI tests at fix commit `b1a50de`. After the Fresh
Evidence Gate passed, the Task 6 four-script suite covering preparation,
native-build logic, final-build logic, and the verifier passed all 268 tests.
The suite did not repeat the expensive native source build.

```bash
rtk node --test \
  scripts/prepare-linux-combined-media-proof.test.ts \
  scripts/build-linux-combined-media-codecs.test.ts \
  scripts/build-linux-combined-media-proof.test.ts \
  scripts/verify-linux-combined-media-proof.test.ts
```

## Gate status

| Gate | Status | Boundary |
| --- | --- | --- |
| Source Gate | PASS | Locked sources, typed archive metadata, prepared-tree revalidation, and notice byte comparisons |
| Native Build Gate | PASS | Pinned Clang/musl/LLVM/OpenH264/libxaac build and 11 runtime inputs |
| Codec Gate | PASS | Direct API bridges, owned fixtures, validation bounds, and cleanup fault harnesses |
| Container Gate | PASS | Owned progressive two-track MP4 path and explicit guard refusals |
| Exact-binary `getrandom` audit | PASS | `ea28d77d` plus offset correction `792df15b`; conditional on all exact predicates |
| Fresh Evidence Gate | PASS | `b1a50de` and attempt 13 passed independent review with no findings or blocked checks in the bounded Task 5b scope |
| Task 6 final independent report review | PASS | Fresh review of `b1a50de` through report commit `298286ef` found no findings; the reviewer independently confirmed 268 passing tests and artifact coverage |

## Scope exclusions and open gates

| Area | Status | What this proof does not establish |
| --- | --- | --- |
| Codec independence | Open | OpenH264 and libxaac each decode their own encoder output |
| Generic conformance | Open | No broad H.264, AAC-LC, or ISO BMFF corpus, malformed corpus, independent parser, or interoperability suite |
| Audio completeness | Open | Central 36,000-frame correlation only; no full-tail, priming/edit, or gapless proof |
| General synchronization | Open | One coded-duration delta and one recovered-content lag; no edits, cuts, clocks, seek, or long-run drift |
| MP4 scope | Open | Edit lists, fragmented MP4, nonzero composition offsets, B-frames, VFR, rotation, `co64`/large media, corruption, and general seek behavior |
| Common production MP4 | Open | The local guard accepts only the owned progressive subset and is not a production backend contract |
| Performance | Open | No production-size media, throughput, latency, memory, cancellation, or concurrency qualification |
| Source obligations | Open | Source-level notices and official-archive membership are not a complete per-file obligation audit |
| Patent analysis | Open | No H.264 or AAC patent-license conclusion |
| Packaging/platform | Open | No packaged application, Linux distribution matrix, installer, update, sandbox, or release artifact qualification |
| Product integration | Out of scope | No production backend routing, canonical project-state change, UI change, or release-readiness field changed |

The result stays bounded to the exact owned fixture, pinned inputs, current
static-musl executable, and recorded conditions. It does not authorize a
production backend choice, UI work, packaging claim, patent conclusion, or
Linux release claim.
