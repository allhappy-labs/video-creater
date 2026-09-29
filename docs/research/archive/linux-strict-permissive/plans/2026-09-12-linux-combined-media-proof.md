# Linux Combined Media Proof Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build one reproducible static-musl CPU proof that generates owned deterministic I420 video and PCM audio, encodes H.264/AAC in-process, muxes progressive MP4, reopens and demuxes it, decodes both streams in-process, and checks predetermined fidelity and timing tolerances.

**Architecture:** Add a standalone development crate under `native/` following the existing Linux probe pattern. Checked-in preparation and build scripts create a fresh private source/toolchain tree from exact Git commits and verified upstream archives, then link direct OpenH264 and libxaac bridges plus `mp4` 0.14.0 into one Rust executable; the executable never searches `PATH` or starts a codec/helper process. A small progressive-container guard accepts only the owned proof output and explicitly rejects edits, fragments, nonzero composition offsets, and inconsistent sample tables, without changing the production common-MP4 requirement.

**Tech Stack:** Rust 1.97.1 (`8bab26f4f68e0e26f0bb7960be334d5b520ea452`), `x86_64-unknown-linux-musl`, `mp4` 0.14.0, OpenH264 2.6.0, libxaac 0.1.13, LLVM runtimes 18.1.8, Clang/LLD 18, Node built-ins for development orchestration, a checked-in host-Python-3 standard-library metadata helper, ELF/link-map inspection.

**Spec:** `docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`

## Global Constraints

- This is the next media feasibility subproject only. Do not route a production backend, change UI, replace GStreamer/GES, change the common H.264/AAC MP4 product requirement, or make a release claim.
- Use direct in-process OpenH264 and libxaac APIs. The proof executable must not invoke FFmpeg, GStreamer, system codecs, shell commands, `PATH` helpers, or any runtime subprocess. Same-process native codec threads are permitted but are not required by this proof.
- Build tools are development-only. Record them separately and prove they do not appear in the final executable or its loaded/runtime file set.
- Create every source checkout, extracted archive, build directory, binary, link map, generated MP4, and raw log from checked-in scripts in a caller-supplied new directory. Do not read any earlier ignored `output/linux-*` tree.
- Pin source inputs by immutable Git commit or upstream archive SHA-256. Pin the official Rust channel manifest and target archives; do not accept whatever happens to be installed by rustup.
- Preserve and package reviewed source/license texts for OpenH264, libxaac, musl, LLVM runtimes, Rust, `mp4`, and the complete locked normal Cargo graph. Treat source terms, binary provenance, functional execution, and release eligibility as separate results.
- The proof uses the same OpenH264 build for encode/decode, the same libxaac build for encode/decode, and the same `mp4` crate for mux/demux. State that clearly: this is integration and provenance evidence, not independent codec/container validation or generic bitstream/format conformance.
- The owned proof profile is 64×48 I420, 30 fps, H.264 constrained-baseline-compatible output, AAC-LC stereo 48 kHz, and a one-second progressive MP4. Any broader profile, B-frame, VFR, edit-list, fragmented-MP4, rotation, `co64`, corrupt-table, seek, or large-`ctts` result remains unqualified.
- Measure AAC decoded length, recovered content lag, coded padding, track duration, and A/V end difference. Do not call the result gapless audio, gapless A/V sync, production synchronization, or general seek correctness.
- Use TDD with meaningful RED/GREEN evidence, exact commands, bounded runs, Conventional Commits, and a fresh independent Sol review at each gate named below. Root coordinates the work; Sol agents own implementation and code/document review.
- Preserve unrelated work and existing ignored evidence. No system package installation, privileged operation, listener, push, merge, new license-policy approval, or release-status change belongs in this plan.

## Accepted starting evidence

- `native/linux-core-probe` at `e24a3896` independently passed five musl tests and a real `video_creater_lib::project::storage` save/reopen round trip. This combined proof does not repeat or extend that canonical project-state test.
- `docs/research/2026-09-12-linux-mp4-timing-proof.md` at `b737c208` independently qualified `mp4` 0.14.0 for narrow progressive `ctts`/`stss` evidence. Edit semantics, fragments, corrupt sample tables, and version-0 `ctts` values above `i32::MAX` remain open.
- `docs/research/2026-09-12-linux-codec-runtime-proof.md` at `35a4d2ed`, with the accepted documentation correction at `07ca2b01`, recorded a four-frame OpenH264 round trip and a 470-frame AAC-LC round trip using source-built LLVM ABI/unwind/CRT objects with no known GNU runtime contribution. Independent review accepted the narrow result while retaining three qualification gaps: twelve contributing rustup `.rlib` files, two transient generated `.rcgu.o` files in the final maps, and no per-file obligation audit of the retained source snapshots.
- `4431322c` makes the exact term `Apache-2.0 WITH LLVM-exception` evaluator-eligible and passes 40 combined tests; its policy/code review is approved. This proof consumes that decision and does not create another license-policy approval.

## Fixed fixture and acceptance values

Keep these constants in `native/linux-combined-media-proof/src/fixture.rs` and reuse them from every test rather than restating numbers in test-only code:

```rust
pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 48;
pub const VIDEO_FRAME_COUNT: usize = 30;
pub const VIDEO_TIMESCALE: u32 = 90_000;
pub const VIDEO_SAMPLE_DURATION: u32 = 3_000;
pub const AUDIO_SAMPLE_RATE: u32 = 48_000;
pub const AUDIO_CHANNELS: usize = 2;
pub const INPUT_PCM_FRAMES: usize = 48_000;
pub const AAC_FRAME_SAMPLES: usize = 1_024;

pub const MAX_Y_MAE: f64 = 18.0;
pub const MAX_CHROMA_MAE: f64 = 22.0;
pub const MIN_AUDIO_CORRELATION: f64 = 0.90;
pub const MAX_AUDIO_LAG_FRAMES: i32 = 2_048;
pub const MIN_AUDIO_RMS_RATIO: f64 = 0.70;
pub const MAX_AUDIO_RMS_RATIO: f64 = 1.30;
pub const MAX_CODED_PADDING_FRAMES: usize = AAC_FRAME_SAMPLES - 1;
pub const MAX_AV_END_DELTA_US: u64 = 21_334; // one 48 kHz AAC frame, rounded up
```

Generate video with full-range deterministic spatial and temporal structure: `Y = (3*x + 5*y + 29*frame + 37*((x/8 + y/8 + frame)%2)) & 255`, `U = 72 + ((11*frame + 3*x + y) % 112)`, and `V = 184 - ((7*frame + x + 2*y) % 112)`, with chroma coordinates used for `x/y`. Generate stereo PCM using integer phase accumulators and ten 4,800-frame segments whose frequency pair changes per segment; left frequencies are `[257, 389, 521, 653, 787, 919, 1051, 1187, 1321, 1453]` Hz and right frequencies are the reverse order plus 83 Hz. Emit a 12,000-amplitude triangle wave with a 2,000-amplitude segment-index marker. Integer generation prevents platform-dependent floating-point fixtures and the changing frequencies make the correlation lag meaningful.

Validate all 30 decoded frames in presentation order. Require exact dimensions and frame count, per-frame luma mean absolute error no greater than `MAX_Y_MAE`, and per-frame U and V mean absolute error no greater than `MAX_CHROMA_MAE`. Compare each decoded frame only with its same-index source frame so reordered or duplicated frames fail.

For audio, require decoded length from 48,000 through 49,023 sample frames. Search integer lags from -2,048 through +2,048 and select the lag with the greatest normalized cross-correlation over the overlapping central 36,000 input frames independently for left and right. Require both correlations at least 0.90, both absolute lags at most 2,048, channel lags within one sample of one another, each channel's RMS ratio from 0.70 through 1.30, and each channel to correlate more strongly with its matching source than the opposite source. Report the actual lag and decoded padding; do not require zero lag.

For the owned MP4, require video duration exactly 1,000,000 microseconds. Require AAC sample durations of 1,024 at a 48,000 timescale and coded padding no greater than 1,023 input PCM frames. Require the absolute difference between the coded audio end and video end no greater than 21,334 microseconds. These are one-fixture coded-duration bounds, not gapless-sync evidence.

---

### Task 1: Pin fresh source and toolchain inputs

**Files:**
- Create: `native/linux-combined-media-proof/Cargo.toml`
- Create: `native/linux-combined-media-proof/Cargo.lock`
- Create: `native/linux-combined-media-proof/src/lib.rs` — empty crate root for locked dependency preparation; Task 3 fills it.
- Create: `native/linux-combined-media-proof/source-lock.json`
- Create: `native/linux-combined-media-proof/notices/README.md`
- Create: `native/linux-combined-media-proof/notices/OPENH264-LICENSE.txt`
- Create: `native/linux-combined-media-proof/notices/LIBXAAC-LICENSE.txt`
- Create: `native/linux-combined-media-proof/notices/LIBXAAC-NOTICE.txt`
- Create: `native/linux-combined-media-proof/notices/LLVM-LIBCXX-LICENSE.txt`
- Create: `native/linux-combined-media-proof/notices/LLVM-LIBCXXABI-LICENSE.txt`
- Create: `native/linux-combined-media-proof/notices/LLVM-LIBUNWIND-LICENSE.txt`
- Create: `native/linux-combined-media-proof/notices/LLVM-COMPILER-RT-LICENSE.txt`
- Create: `native/linux-combined-media-proof/notices/MUSL-COPYRIGHT.txt`
- Create: `native/linux-combined-media-proof/notices/RUST-COPYRIGHT.txt`
- Create: `native/linux-combined-media-proof/notices/MP4-LICENSE.txt`
- Create: `scripts/linux-combined-input-metadata.py`
- Create: `scripts/prepare-linux-combined-media-proof.mjs`
- Create: `scripts/prepare-linux-combined-media-proof.test.ts`

**Interfaces:**
- `source-lock.json` is the sole machine-readable native/toolchain input inventory; `Cargo.lock` is the sole Rust package-resolution inventory. Each source-lock entry has `name`, `kind`, `source`, `revision` or `version`, `sha256` where it is an archive, `licenseExpression`, and one or more `noticePaths`. An archive may set `retainOnly: true`; preparation then verifies and retains the original archive but does not extract it or expose payload paths.
- `prepareCombinedMediaInputs({ output, lockPath, fetchArchive, runGit, runTool }) -> Promise<PreparedInputs>` refuses a pre-existing output path, creates only beneath it, verifies every archive before extraction, verifies every Git `HEAD`, and writes `prepared-inputs.json` containing resolved paths and hashes. A retain-only input is represented only by its retained archive path and hash, never by an extracted root or executable path.
- Run `scripts/linux-combined-input-metadata.py` only through a resolved absolute host Python 3 with `shell: false`. Its `archive --input ABSOLUTE_TAR` mode uses `tarfile` and emits versioned typed JSON entries `{ path, kind, linkTarget, size }`; its `rust-channel --input ABSOLUTE_TOML --release 1.97.1 --host x86_64-unknown-linux-gnu --target x86_64-unknown-linux-musl` mode uses `tomllib` and emits the exact selected rustc, Cargo, host-std, and musl-std `{ package, target, url, hash }` records from their respective TOML tables. The Node orchestrator validates the JSON schema, requires each selected URL including its filename and its paired `xz_hash` to equal the corresponding lock entry, and never trusts verbose `tar` listing text or substring searches in TOML.
- `revalidatePreparedInputs(...)` must rehash the checked-in lock and every retained archive, retained Cargo crate, assembled toolchain/musl inventory file, selected musl-target `.rlib`, and recorded build-tool executable; recheck exact Cargo-lock/checksum bindings, Rust archive membership, tool versions, notice bytes, Git revisions/cleanliness, and all retained-root constraints before downstream use.
- CLI: `node scripts/prepare-linux-combined-media-proof.mjs --lock ABSOLUTE_FILE --manifest ABSOLUTE_CARGO_TOML --output NEW_DIRECTORY`, plus `--validate-lock` for the download-free schema check.

- [x] **Step 1: Write preparation RED tests**

Use injected fetch/Git/tool runners so unit tests make no network request. Cover: a valid archive hash is accepted; one changed byte is rejected before extraction; a Git checkout whose `HEAD` differs is rejected; a pre-existing output directory and symlink are refused without changing a sentinel; a retain-only archive is hashed and retained without an extraction call or exposed payload path; typed archive JSON preserves a regular filename containing ` -> ` while an absolute or escaping typed symlink/hardlink target is rejected before extraction; exact TOML-key lookup rejects a changed selected URL/hash even when the expected text appears under another package or target; and no path in the resulting manifest contains `output/linux-codec-runtime-proof`, `output/linux-media-feasibility`, or `output/linux-mp4-timing-proof`.

Add mutation tests proving revalidation fails independently after changing one assembled toolchain file, one assembled musl file, one selected target `.rlib`, `Cargo.lock`, one retained `.crate`, or one recorded build-tool executable. The test fixtures must exercise typed helper JSON; verbose listing text and hash-substring matching are not accepted fixture interfaces.

```js
test("rejects a source archive before extraction when sha256 differs", async (t) => {
  const root = await temporaryDirectory(t);
  const calls = [];
  await assert.rejects(
    prepareCombinedMediaInputs({
      output: join(root, "new-output"),
      lockPath: fixtureLock(root, { sha256: "00".repeat(32) }),
      fetchArchive: async () => Buffer.from("changed"),
      runGit: async () => { throw new Error("git must not run"); },
      runTool: async (...args) => calls.push(args),
    }),
    /sha256 mismatch/,
  );
  assert.deepEqual(calls, []);
});
```

- [x] **Step 2: Run the preparation tests and capture RED**

Run: `rtk node --test scripts/prepare-linux-combined-media-proof.test.ts`

Expected: FAIL because `prepare-linux-combined-media-proof.mjs` and `prepareCombinedMediaInputs` do not exist.

- [x] **Step 3: Define the exact input lock**

The lock must contain these immutable sources. During plan review, a live fetch of `channel-rust-1.97.1.toml` produced the listed manifest SHA-256 and the manifest itself supplied the four listed Rust archive hashes; Task 1 repeats and retains that verification.

| Input | Exact locator | Verification |
| --- | --- | --- |
| OpenH264 | `https://github.com/cisco/openh264.git` | commit `652bdb7719f30b52b08e506645a7322ff1b2cc6f` |
| libxaac | `https://github.com/ittiam-systems/libxaac.git` | commit `55af74f9ce7cadb99fbd7870c6487c8a12d75689` |
| LLVM project | `https://github.com/llvm/llvm-project.git` | commit `3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff` |
| Rust 1.97.1 channel manifest | `https://static.rust-lang.org/dist/channel-rust-1.97.1.toml` | SHA-256 `03569b1886ceb5c05276b50c8431ab111de944cd6140fe1fa7d821dd8e0f29cf` |
| Rustc host archive | `https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-gnu.tar.xz` | SHA-256 `9819d0a32d56bd339585319c80260e332779f5541fd66838ab7e016d6c814819` |
| Cargo host archive | `https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-gnu.tar.xz` | SHA-256 `e1be5f5ff7f7f80ca506fb65770b759edbdc6d303781ed71c5de8ec8a8394779` |
| Rust host standard library | `https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-gnu.tar.xz` | SHA-256 `1c1e704ae80126b7de34f72ea2825f7fd01736dec20732faed47374b95282fba` |
| Rust musl standard library | `https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.xz` | SHA-256 `51d83178680556f73a5fa8ad865b76a1ff541867445c00fc65dc67246bc2de66` |
| Clang 18 | Ubuntu noble `clang-18` `1:18.1.3-1ubuntu1`, amd64 | SHA-256 `628b16701014ef7ad648380b20ea74b90dd543857f933b3e34d2fc042783de25` |
| Clang headers | Ubuntu noble `libclang-common-18-dev` `1:18.1.3-1ubuntu1`, amd64 | SHA-256 `d41631bb752815a8196261d1030c096591c59678c8adfbf2e6a740a691b49e80` |
| LLD 18 | Ubuntu noble `lld-18` `1:18.1.3-1ubuntu1`, amd64 | SHA-256 `c574594d714bf445f4c2323fa99374c05b79be09a3899edbf9d4b7265c9ccc57` |
| LLVM linker tools | Ubuntu noble `llvm-18-linker-tools` `1:18.1.3-1ubuntu1`, amd64 | SHA-256 `50f0bcfa784e42f8bf0eac31aeb6a20798fb6174cc31efe021f12ae71af91b23` |
| musl runtime | Ubuntu noble `musl` `1.2.4-2`, amd64 | SHA-256 `9f0883c20b4b746e05e947bafd99cb933f5494ffaaa6fcd360cbe1fbcf264883` |
| musl headers/CRT | Ubuntu noble `musl-dev` `1.2.4-2`, amd64 | SHA-256 `4b451ecb6a0f8469883058cf22a807f3bd9cc16d115cc08b7efc35fe8eb44db2` |
| musl wrapper metadata | Ubuntu noble `musl-tools` `1.2.4-2`, amd64 | SHA-256 `46c01d212d3eb3a1322693089037f0a5c92383a089d39c392db3c86c19ffb229`; `retainOnly: true` because its `musl-ldd` payload is an absolute symlink |
| `mp4` crate | crates.io `mp4` 0.14.0 / upstream commit `18f1718b70955e2982f81b9827aa396b533c4fbc` | crate SHA-256 `c9ef834d5ed55e494a2ae350220314dc4aacd1c43a9498b00e320e0ea352a5c3` |

Use HTTPS Ubuntu pool URLs matching those exact package/version/architecture values. The four Clang/LLD packages are development executables and headers; their host dynamic libraries remain build-host prerequisites and must be labeled as such. They are not runtime-closure inputs.

- [x] **Step 4: Implement the preparation script and notice snapshots**

Use Node built-ins for hashing, download, path validation, and process launch. Use the checked-in Python helper only for typed tar metadata and exact Rust-channel TOML selection. Resolve the existing host Python 3 to an absolute path, require the standard-library modules `tarfile` and `tomllib`, and record its path, version output, and executable SHA-256 as a development-only build tool; block if it is unavailable or incompatible, and do not install Python or include it in runtime inputs. Invoke `git`, extraction-only `tar`, and `dpkg-deb` only in this development script with `shell: false`; record their absolute resolved paths, version output, and SHA-256 where they are regular files. Use `git init`, `git remote add`, `git fetch --depth=1 origin <commit>`, `git checkout --detach FETCH_HEAD`, then require `git rev-parse HEAD` to equal the lock. For every archive selected for extraction, consume the helper's typed member JSON, validate its schema, and reject absolute paths, `..` components, device/unknown entry kinds, and absolute or escaping symlink/hardlink targets before joining them to a destination path; do not parse or trust verbose `tar` text. Hash and retain the locked `musl-tools` `.deb` without extracting it or exposing `musl-gcc`/`musl-ldd`; Task 2 needs only the extracted musl runtime and musl-dev sysroot with direct Clang. This retain-only exception does not relax link validation for any archive that is extracted. Assemble the four verified Rust archives into `<output>/rust`, retain a path/hash inventory for every assembled toolchain and musl file, require its `rustc -Vv` and `cargo -Vv` output to match the locked commits, and require every target `.rlib` used later to be byte-identical to a member of the verified musl standard-library archive. Retain the Rust license/copyright files shipped in the verified archives; no full Rust Git checkout is needed.

Create the standalone Cargo manifest now with edition 2024, `publish = false`, a local `[workspace]`, `native-codecs = []`, release `panic = "unwind"`, LTO, one codegen unit, and `mp4 = { version = "=0.14.0", default-features = false }`. Generate and commit its lockfile using the pinned private Cargo. During preparation, set `RUSTC=<output>/rust/bin/rustc` and `CARGO_HOME=<output>/cargo-home`, run `cargo fetch --manifest-path <manifest> --locked --target x86_64-unknown-linux-musl`, retain every downloaded `.crate`, checksum, and license file, and prove `cargo metadata --locked --offline` succeeds. Release unwind is intentional because Task 4 contains the known `mp4` parser panic; source-built `libunwind.a` supplies the proof's unwinder.

Copy the exact reviewed license/copyright content into the checked-in notice files, including libxaac's attribution NOTICE. Because Task 2 consumes headers from pinned `libcxx/include` even though it does not build or link `libc++.a`, preserve `libcxx/LICENSE.TXT` separately as `LLVM-LIBCXX-LICENSE.txt`. Retain the actual selected libc++abi, libunwind, and compiler-rt component texts in their own files as well. Make `notices/README.md` map every snapshot to its locked source path and license expression. The checked-in copies are review inputs; the preparation script must also compare each one byte-for-byte with the freshly fetched source or package metadata.

- [x] **Step 5: Run preparation tests GREEN and validate the lock without downloads**

Run: `rtk node --test scripts/prepare-linux-combined-media-proof.test.ts`

Expected: PASS for all local injected-runner cases.

Run: `rtk node scripts/prepare-linux-combined-media-proof.mjs --lock "$PWD/native/linux-combined-media-proof/source-lock.json" --manifest "$PWD/native/linux-combined-media-proof/Cargo.toml" --validate-lock`

Expected: `COMBINED_MEDIA_SOURCE_LOCK_VALID` after schema, duplicate-name, digest-shape, notice-path, and forbidden-retained-path checks.

- [x] **Step 6: Prepare the fresh inputs once for downstream tasks**

```bash
rtk node scripts/prepare-linux-combined-media-proof.mjs \
  --lock "$PWD/native/linux-combined-media-proof/source-lock.json" \
  --manifest "$PWD/native/linux-combined-media-proof/Cargo.toml" \
  --output "$PWD/output/linux-combined-media-proof/inputs"
```

Expected: every source/archive and notice comparison passes; exact helper-selected Rust TOML fields match the lock; private rustc/Cargo report the locked revisions; offline Cargo metadata passes; a final `revalidatePreparedInputs` pass rechecks the complete assembled toolchain/musl inventories, selected `.rlib` membership, Cargo lock/crates/checksums, and recorded tool hashes; `prepared-inputs.json` contains no earlier proof path.

- [x] **Step 7: Commit and pass Source Gate review**

```bash
rtk git add native/linux-combined-media-proof/Cargo.toml native/linux-combined-media-proof/Cargo.lock native/linux-combined-media-proof/src/lib.rs native/linux-combined-media-proof/source-lock.json native/linux-combined-media-proof/notices scripts/linux-combined-input-metadata.py scripts/prepare-linux-combined-media-proof.mjs scripts/prepare-linux-combined-media-proof.test.ts
rtk git commit -m "build(linux): pin combined media proof inputs"
```

Source Gate reviewer: a fresh Sol agent independently checks every revision/hash against the cited upstream source, confirms typed archive parsing and exact Rust TOML table/URL/hash bindings, exercises complete revalidation mutation coverage, confirms notice-byte comparisons, confirms no earlier ignored output path is consumed, and reports build-only tools separately from candidate runtime inputs. Stop this subproject on any mismatch; fixing the lock requires another Source Gate review.

### Task 2: Check in and test the LLVM/musl/native-codec build recipe

**Files:**
- Create: `native/linux-combined-media-proof/build-support/linker.sh`
- Create: `scripts/build-linux-combined-media-codecs.mjs`
- Create: `scripts/build-linux-combined-media-codecs.test.ts`

**Interfaces:**
- `buildCombinedMediaCodecs({ preparedManifest, preparedPath, output, run, signal }) -> Promise<NativeBuildManifest>` revalidates Task 1's manifest against the exact checked-in source lock before use, accepts one new native root, writes source-built artifacts beneath `<output>/lib`, and writes `native-build.json` with every archive/CRT path, SHA-256, source revision, build flags, and notice path. Its default process runner delegates to Task 1's owned-process runner, and its CLI maps `SIGINT`/`SIGTERM` to the shared abort signal.
- `linker.sh` requires absolute `VC_COMBINED_TOOLCHAIN_ROOT`, `VC_COMBINED_MUSL_ROOT`, `VC_COMBINED_NATIVE_ROOT`, and `VC_COMBINED_LINK_MAP_DIR` values. `VC_COMBINED_NATIVE_ROOT` is the Task 2 output itself, with archives under its `lib` directory. The wrapper replaces Rust target-distribution `rcrt1.o`, `crti.o`, `crtn.o`, `crtbegin*.o`, and `crtend*.o` arguments with verified musl and source-built compiler-rt files, derives a unique map filename from the linker's `-o` output, safely creates the absent map directory beneath its existing parent, and writes that exact path to `<link-output>.link-map-path` after the pinned private Clang/LLD driver succeeds.
- Symbol inventory uses the resolved absolute GNU `/usr/bin/nm` with `--undefined-only`, matching Task 5's absolute `--nm /usr/bin/nm` interface. The recipe records its resolved path, exact `--version` output, and executable SHA-256 under development-only `buildTools`; it rejects a non-GNU version or unrecognized inventory line format and never lists `nm` as a target runtime input. The only permitted exit-zero diagnostic is GNU nm's exact `<member>: no symbols` form for a static archive: require the named member in that exact archive, extract and hash it, use the recorded absolute `readelf` to prove it is an ELF with no symbol-table section, and retain the diagnostic plus inspection evidence in `native-build.json`. Reject nonzero nm, unknown/mismatched members, every other diagnostic, and any inspected member containing `.symtab` or `.dynsym`. Do not install another package solely to obtain `nm` or `readelf`.
- CLI: `node scripts/build-linux-combined-media-codecs.mjs --prepared ABSOLUTE_JSON --output NEW_DIRECTORY`.

- [x] **Step 1: Write build-recipe RED tests**

Inject a recording `run` function and assert the command plan contains the target triple, private sysroot, source roots from revalidated `prepared-inputs.json`, `USE_ASM=No`, and the exact LLVM/runtime flags below. Assert it never contains `/usr/lib/gcc`, `-lstdc++`, a previous proof output path, or an unqualified executable name after input resolution. Add fixtures proving rejection of a changed lock hash, supplied revision/package-hash/root drift, and dirty tracked or untracked source checkouts. Exercise the shared owned-process runner with real descendants and prove timeout, output-limit, and abort cleanup preserve an unrelated process. Add fixtures proving that recognized GNU `nm --version` plus undefined-symbol output pass, while a non-GNU version and a malformed inventory line fail before the manifest is written.

```js
test("plans only the pinned musl and LLVM runtime closure", async () => {
  const commands = [];
  await buildCombinedMediaCodecs({
    preparedManifest: verifiedFixtureManifest,
    output: newOutput,
    run: async (command) => { commands.push(command); return successfulFixtureResult(command); },
  });
  const text = JSON.stringify(commands);
  assert.match(text, /x86_64-unknown-linux-musl/);
  assert.match(text, /LIBCXXABI_USE_LLVM_UNWINDER/);
  assert.doesNotMatch(text, /(?:libstdc\+\+|\/usr\/lib\/gcc|linux-codec-runtime-proof)/);
});
```

- [x] **Step 2: Run build-recipe tests and capture RED**

Run: `rtk node --test scripts/build-linux-combined-media-codecs.test.ts`

Expected: FAIL because the build module does not exist.

- [x] **Step 3: Implement the source-built LLVM runtime/CRT recipe**

Translate the retained successful recipe into path-independent commands rooted only in the revalidated Task 1 manifest and Task 2 output. Treat the checked-in source lock as the sole source inventory: reread its actual bytes, verify the manifest's lock path/hash and every retained identity, constrain retained source/toolchain/musl/Rust paths to the prepared root, and require clean Git source checkouts before any build or copy. Configure LLVM `runtimes/` for `libcxx;libcxxabi;libunwind` so libc++ supplies its generated headers, using release, static-only, musl mode, compiler-rt, LLVM unwinder, no tests, and at most three build jobs. Build only the `cxxabi` and `unwind` targets and retain only those two archives, the artifacts proven to contribute. Fail if either archive is absent and retain the known full-`libc++.a` toolchain-completeness gap without building the unused archive.

Compile pinned `compiler-rt/lib/builtins/crtbegin.c` and `crtend.c` with:

```text
--target=x86_64-unknown-linux-musl
--sysroot=<prepared musl root>
-isystem <prepared musl root>/usr/include/x86_64-linux-musl
-std=c11 -DCRT_HAS_INITFINI_ARRAY -DEH_USE_FRAME_REGISTRY
-fPIC -Wno-pedantic -O2
```

- [x] **Step 4: Implement the codec archive recipe**

Build the source-lock entries named `openh264` and `libxaac` at their revalidated revisions. Use the private Clang target driver and libc++ headers for OpenH264, with `USE_ASM=No`, release mode, and no install step; build only `libopenh264.a`. Build libxaac through CMake with the same target/sysroot and build only `libxaacenc.a` and `libxaacdec.a`. Do not compile or link either upstream command-line testbench; Task 3 calls the public codec APIs directly.

After each build, require the source `HEAD`, hash every output archive, and run the resolved absolute GNU `/usr/bin/nm --undefined-only`. Require its recorded version to identify GNU nm and every nonblank inventory line to match the expected GNU archive/object header or undefined-symbol form; reject diagnostics or unparsed output rather than treating it as symbol evidence. Record the tool's path, exact version output, executable hash, and full compiler/link commands. Fail if command text or symbol/archive inventory contains `libstdc++`, `libgcc`, glibc, or GCC `crtstuff` paths. GNU `nm` and development Clang are recorded in `native-build.json` under `buildTools`, never under `runtimeInputs`; they contribute nothing to the target executable.

- [x] **Step 5: Implement the final-link wrapper contract**

The checked-in wrapper must filter only target-distribution CRT basenames and substitute verified paths from the current build root. It must reject missing/non-absolute roots and append `--target=x86_64-unknown-linux-musl`, `--sysroot`, `-fuse-ld=lld`, the source-built LLVM archive directory, and the musl archive directory. It must retain unique-per-output map names and expose the exact selected map through `<link-output>.link-map-path`; consumers read this record rather than guessing the hash. It must not contain a developer home path.

- [x] **Step 6: Run build-recipe tests GREEN**

Run: `rtk node --test scripts/build-linux-combined-media-codecs.test.ts`

Expected: PASS, including manifest/lock/root/source-integrity refusal, owned-descendant cleanup, substitution, unique-per-output map naming and exact-path handoff, path-refusal, forbidden-input, GNU-nm version/output-format validation, and manifest-completeness cases.

- [x] **Step 7: Execute the native build once and retain its manifest**

```bash
rtk node scripts/build-linux-combined-media-codecs.mjs \
  --prepared "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs.json" \
  --output "$PWD/output/linux-combined-media-proof/native-build-attempt4"
```

Expected: `native-build.json` names source-built `libopenh264.a`, `libxaacenc.a`, `libxaacdec.a`, `libc++abi.a`, `libunwind.a`, `crtbeginS.o`, and `crtendS.o`, with exact hashes and no known GNU runtime input.

- [x] **Step 8: Commit and pass Runtime Recipe Gate review**

```bash
rtk git add native/linux-combined-media-proof/build-support/linker.sh scripts/build-linux-combined-media-codecs.mjs scripts/build-linux-combined-media-codecs.test.ts
rtk git commit -m "build(linux): reproduce permissive codec runtime"
```

Runtime Recipe Gate reviewer: a fresh Sol agent compares the checked-in flags and substitution logic to `docs/research/2026-09-12-linux-codec-runtime-proof.md`, checks that direct codec libraries rather than testbench executables are outputs, and confirms every generated path is rooted in the current caller-supplied tree.

Task 2 is complete: the code/test review and the actual recipe/artifact review passed against Source-Gate-approved inputs. The accepted retained result is `output/linux-combined-media-proof/native-build-attempt4/native-build.json`, SHA-256 `ce89fb8f9fe9bc5584846db14cf7d294c83983242157eebf5af9efef335f263e`.

### Task 3: Add direct in-process codec bridges and deterministic validation

**Files:**
- Modify: `native/linux-combined-media-proof/Cargo.toml` — add only `[build-dependencies] serde_json = "=1.0.151"` for strict `native-build.json` parsing; this exact package is already in the approved 22-package registry inventory.
- Modify: `native/linux-combined-media-proof/Cargo.lock` — add only the `serde_json` root dependency edge. The lock hash changes from `a1a1e23b63d8e932075c2a39682e527cad077bd6d85fba69176b534116ab4b48` to `fe666c3b994c4287553717924827cc9646dd1b1c0128d85df5e595e956c4c28c`; all registry package versions and checksums remain unchanged.
- Modify: `scripts/prepare-linux-combined-media-proof.mjs` — add the bounded Cargo-only snapshot derivation operation.
- Modify: `scripts/prepare-linux-combined-media-proof.test.ts` — prove exact old/new locks, unchanged inventories, and strong derived-snapshot revalidation.
- Create: `native/linux-combined-media-proof/build.rs`
- Modify: `native/linux-combined-media-proof/src/lib.rs`
- Create: `native/linux-combined-media-proof/src/codec.rs`
- Create: `native/linux-combined-media-proof/src/fixture.rs`
- Create: `native/linux-combined-media-proof/src/validate.rs`
- Create: `native/linux-combined-media-proof/native/codec_bridge.h`
- Create: `native/linux-combined-media-proof/native/h264_bridge.cpp`
- Create: `native/linux-combined-media-proof/native/aac_bridge.c`
- Create: `native/linux-combined-media-proof/native/tests/aac_decoder_cleanup_test.c` — test-only partial-initialization fault harness.
- Create: `native/linux-combined-media-proof/native/tests/codec_owner_test.cpp` — test-only exception-path ownership harness.

**Interfaces:**

- `deriveTask3CargoSnapshot({ basePreparedPath, preparedPath, manifest, lockPath, signal, runTool? })` requires immutable base `output/linux-combined-media-proof/inputs/prepared-inputs.json` with SHA-256 `ed1be347b5d8155ca5d41002973db948f6a9ea574352473a5a537d331f86e170`, requires the new prepared path to be its exact sibling `prepared-inputs-task3.json`, and creates deterministic `inputs/cargo-project-task3`. It preserves the original prepared snapshot and private bootstrap bytes, records the base hash and exact old/new Cargo-lock hashes, proves the exact `serde_json` root build edge with the same 22 registry packages/versions/checksums/cache hashes and unchanged source/toolchain/musl/rlib inventories, then strongly revalidates only the derived snapshot.
- CLI: `node scripts/prepare-linux-combined-media-proof.mjs --derive-task3-cargo-from ABS_BASE_PREPARED --task3-prepared ABS_NEW_SNAPSHOT --lock ABS_SOURCE_LOCK --manifest ABS_TASK3_CARGO_TOML`. This checked-in operation is the only accepted derivation for the retained Task 1/2 evidence. Keep the original base prepared snapshot, `inputs/cargo-project`, and accepted Task 2 attempt immutable; do not rebuild native archives.
- A full fresh preparation from the latest checked-in Cargo lock produces a current prepared snapshot directly and needs no derivation. The derivation exception must not become a general lock-drift allowance.

```rust
pub struct I420Frame { pub y: Vec<u8>, pub u: Vec<u8>, pub v: Vec<u8> }
pub struct VideoAccessUnit { pub avcc: Vec<u8>, pub is_sync: bool }
pub struct EncodedVideo { pub sps: Vec<u8>, pub pps: Vec<u8>, pub samples: Vec<VideoAccessUnit> }
pub struct EncodedAudio { pub samples: Vec<Vec<u8>>, pub coded_frames: usize }
pub struct DecodedAudio { pub interleaved: Vec<i16>, pub sample_rate: u32, pub channels: u8 }
pub struct AacLcConfig { pub object_type: u8, pub sample_rate: u32, pub channels: u8 }
pub struct VideoMetrics { pub decoded_frames: usize, pub max_y_mae: f64, pub max_u_mae: f64, pub max_v_mae: f64 }
pub struct AudioMetrics {
    pub decoded_frames: usize,
    pub padding_frames: usize,
    pub left_lag: i32,
    pub right_lag: i32,
    pub left_correlation: f64,
    pub right_correlation: f64,
    pub left_rms_ratio: f64,
    pub right_rms_ratio: f64,
}

pub fn owned_i420_frames() -> Vec<I420Frame>;
pub fn owned_pcm() -> Vec<i16>;
pub fn encode_h264(frames: &[I420Frame]) -> Result<EncodedVideo, CodecError>;
pub fn decode_h264(sps: &[u8], pps: &[u8], samples: &[VideoAccessUnit]) -> Result<Vec<I420Frame>, CodecError>;
pub fn encode_aac_lc(interleaved: &[i16]) -> Result<EncodedAudio, CodecError>;
pub fn decode_aac_lc(config: AacLcConfig, samples: &[Vec<u8>]) -> Result<DecodedAudio, CodecError>;
pub fn validate_video(expected: &[I420Frame], actual: &[I420Frame]) -> Result<VideoMetrics, ValidationError>;
pub fn validate_audio(expected: &[i16], actual: &DecodedAudio) -> Result<AudioMetrics, ValidationError>;
```

The C ABI owns no Rust allocation. Each bridge result uses bridge-allocated arrays plus a matching `vc_*_free` function; Rust immediately copies into owned vectors and frees through an RAII guard on success and error. The bridge returns numeric status plus a bounded UTF-8 error buffer. No exception may cross the C ABI.

- [x] **Prerequisite: Derive and independently review the Task 3 Cargo-only prepared snapshot**

```bash
rtk node scripts/prepare-linux-combined-media-proof.mjs \
  --derive-task3-cargo-from "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs.json" \
  --task3-prepared "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs-task3.json" \
  --lock "$PWD/native/linux-combined-media-proof/source-lock.json" \
  --manifest "$PWD/native/linux-combined-media-proof/Cargo.toml"
```

Expected: the original prepared snapshot remains SHA-256 `ed1be347b5d8155ca5d41002973db948f6a9ea574352473a5a537d331f86e170`; the derived snapshot records the approved old/new lock hashes, the one new root build edge, unchanged 22-package registry and native/source inventories, and a complete strong revalidation pass. A fresh independent supplemental review checks those facts before native bridge tests consume the derived snapshot.

- [x] **Step 1: Write pure fixture RED tests before enabling native bridges**

Keep the Task 1 manifest, release unwind setting, and locked registry checksums. Permit only the exact existing `serde_json = "=1.0.151"` build dependency and its single new root lock edge; forbid a handwritten JSON parser, another package/version, or any registry version/checksum change. Gate native FFI/link work behind the nondefault `native-codecs` feature so pure fixture/metric tests run without a native archive. `build.rs` returns immediately without that feature; with it, the script parses the absolute verified JSON in `VC_COMBINED_NATIVE_MANIFEST`, requires every archive beneath `VC_COMBINED_NATIVE_ROOT`, compiles the two checked-in bridges with the pinned compiler, and emits static link directives for OpenH264, libxaac encoder/decoder, libc++abi, and libunwind.

```rust
#[test]
fn owned_fixture_has_exact_shape_and_is_reproducible() {
    let first = owned_i420_frames();
    let second = owned_i420_frames();
    assert_eq!(first, second);
    assert_eq!(first.len(), VIDEO_FRAME_COUNT);
    assert!(first.windows(2).all(|pair| pair[0] != pair[1]));
    let pcm = owned_pcm();
    assert_eq!(pcm.len(), INPUT_PCM_FRAMES * AUDIO_CHANNELS);
    assert!(pcm.chunks_exact(2).any(|sample| sample[0] != sample[1]));
}
```

- [x] **Step 2: Run the pure fixture test and capture RED**

Run: `rtk env RUSTC="$PWD/output/linux-combined-media-proof/inputs/rust/bin/rustc" CARGO_HOME="$PWD/output/linux-combined-media-proof/inputs/cargo-home" "$PWD/output/linux-combined-media-proof/inputs/rust/bin/cargo" test --manifest-path native/linux-combined-media-proof/Cargo.toml --locked --offline --no-default-features owned_fixture_has_exact_shape_and_is_reproducible`

Expected: FAIL because `owned_i420_frames` and `owned_pcm` are absent or do not yet satisfy the exact fixture assertions. No native build manifest is consulted in this test.

- [x] **Step 3: Implement deterministic fixture generation and metric functions**

Implement the formulas and constants from “Fixed fixture and acceptance values.” Video metrics calculate each plane's mean absolute error per frame. Audio metrics use `i128` accumulators for dot products/energy, ignore zero-overlap candidates, and compare matching and crossed channels over the same 36,000-frame window. Return actual per-frame MAE, lag, correlation, RMS ratio, decoded length, and padding for the final report.

Add boundary tests that deliberately duplicate a video frame, swap audio channels, add 2,049 leading audio frames, and exceed each MAE/correlation threshold. Each must return a named `ValidationError`; these tests establish that the acceptance test can fail for wrong content.

- [x] **Step 4: Implement the OpenH264 direct bridge**

Use `WelsCreateSVCEncoder`, `ISVCEncoder::Initialize`, `SetOption(ENCODER_OPTION_DATAFORMAT)`, and `EncodeFrame` for 30 source pictures. Configure 64×48 I420, 30 fps, CPU, `USE_ASM=No` build, 512,000 bit/s, and no B-frame/reordering mode. Parse every returned layer's NAL lengths, remove Annex-B start codes, retain the first type-7 SPS and type-8 PPS, serialize remaining NALs as four-byte big-endian-length AVCC samples, and set `is_sync` only for an IDR access unit. Require 30 nonempty access units and a first sync sample.

Decode by reconstructing Annex-B in memory from demuxed SPS/PPS plus each AVCC access unit, call `WelsCreateDecoder`, `Initialize`, and `DecodeFrameNoDelay`, copy decoded planes respecting returned strides, and flush until no delayed frame remains. No file or command-line codec interface belongs in this bridge.

- [x] **Step 5: Implement the libxaac direct bridge**

Port only the allocation/configuration/process lifecycle demonstrated by the pinned upstream testbenches. Encoder calls `ixheaace_create`, `ixheaace_process`, and `ixheaace_delete` directly with AOT 2, 48 kHz, stereo, 128,000 bit/s, 16-bit interleaved PCM, and transport-header output disabled. Feed exactly 48,000 owned PCM frames, flush with the upstream end-of-input contract, and return each raw AAC access unit separately with `coded_frames = access_units * 1_024`.

Decoder calls `ia_xheaacd_dec_api` directly through its documented pre-config/init/execute commands. Configure it from the demuxed `AacLcConfig { object_type: 2, sample_rate: 48_000, channels: 2 }`, feed one raw access unit at a time, flush, and return interleaved PCM. Require 16-bit stereo 48 kHz output. The bridge must not rename/call a testbench `main`, construct argv, or read/write an ADTS/WAV file.

- [x] **Step 6: Write and run in-memory codec RED/GREEN tests**

Compile and run the two test-only native fault harnesses with the same static-musl closure. `aac_decoder_cleanup_test.c` must exercise cleanup after partial AAC initialization, including `INPUT_OVER` with a null state; `codec_owner_test.cpp` must exercise H.264 handle cleanup when the bridge path throws. These harnesses are failure-coverage evidence only and are not linked into the proof executable.

```rust
#[test]
fn direct_codecs_round_trip_owned_media_with_declared_tolerances() {
    let video = owned_i420_frames();
    let encoded_video = encode_h264(&video).expect("encode H.264");
    let decoded_video = decode_h264(
        &encoded_video.sps,
        &encoded_video.pps,
        &encoded_video.samples,
    ).expect("decode H.264");
    validate_video(&video, &decoded_video).expect("video tolerances");

    let pcm = owned_pcm();
    let encoded_audio = encode_aac_lc(&pcm).expect("encode AAC-LC");
    assert!(encoded_audio.coded_frames >= INPUT_PCM_FRAMES);
    assert!(encoded_audio.coded_frames - INPUT_PCM_FRAMES <= MAX_CODED_PADDING_FRAMES);
    let decoded_audio = decode_aac_lc(AacLcConfig::stereo_48k(), &encoded_audio.samples)
        .expect("decode AAC-LC");
    validate_audio(&pcm, &decoded_audio).expect("audio tolerances");
}
```

Run after Task 1 preparation and Task 2 native build:

```bash
rtk mkdir -p "$PWD/output/linux-combined-media-proof/test-link-maps"

rtk env \
  VC_COMBINED_NATIVE_MANIFEST="$PWD/output/linux-combined-media-proof/native-build-attempt4/native-build.json" \
  VC_COMBINED_NATIVE_ROOT="$PWD/output/linux-combined-media-proof/native-build-attempt4" \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$PWD/native/linux-combined-media-proof/build-support/linker.sh" \
  VC_COMBINED_TOOLCHAIN_ROOT="$PWD/output/linux-combined-media-proof/inputs/toolchain" \
  VC_COMBINED_MUSL_ROOT="$PWD/output/linux-combined-media-proof/inputs/musl" \
  VC_COMBINED_LINK_MAP_DIR="$PWD/output/linux-combined-media-proof/test-link-maps" \
  RUSTC="$PWD/output/linux-combined-media-proof/inputs/rust/bin/rustc" \
  CARGO_HOME="$PWD/output/linux-combined-media-proof/inputs/cargo-home" \
  RUSTFLAGS="-C save-temps" \
  "$PWD/output/linux-combined-media-proof/inputs/rust/bin/cargo" test --manifest-path native/linux-combined-media-proof/Cargo.toml --locked --offline --features native-codecs --target x86_64-unknown-linux-musl direct_codecs_round_trip_owned_media_with_declared_tolerances -- --nocapture
```

Expected GREEN: 10 pure tests, 11 full-musl tests, and both static-musl fault harnesses PASS; 30 decoded frames pass the per-plane limits; decoded audio length, lag, matching-channel correlation, and RMS ratio pass the unchanged fixed bounds. The harness binaries have no `NEEDED` entry, which qualifies only those harnesses and does not qualify the future final proof ELF. Record actual values; do not weaken constants to accommodate an unexplained failure.

- [x] **Step 7: Commit and pass Codec Gate review**

```bash
rtk git add native/linux-combined-media-proof scripts/prepare-linux-combined-media-proof.mjs scripts/prepare-linux-combined-media-proof.test.ts
rtk git commit -m "feat(linux): add in-process codec proof"
```

Codec Gate reviewer: a fresh Sol agent checks FFI ownership/error paths, confirms the public codec APIs are called directly, verifies AVCC/Annex-B conversion and SPS/PPS treatment, and checks that every tolerance is declared before the combined container run. Any tolerance change requires a reason and a repeated independent Codec Gate review.

Task 3 is complete at `9659dba5`: the same independent reviewer accepted all three requested fixes, 10 pure tests, 11 full-musl tests, and both fault harnesses with unchanged metrics. The retained ignored Task 3 report has SHA-256 `f213f794cfcf81363542f49337d70e77b0e9ab21c3d5dfac38959fc0abe998b6`.

### Task 4: Mux, guard, reopen, demux, and decode the owned MP4

Status: active with a fresh Sol worker from base `9659dba5`; no Task 4 step is complete yet.

**Files:**
- Modify: `native/linux-combined-media-proof/src/lib.rs` — register and export only the Task 4 `container` and `progressive_guard` modules/items needed by the integration test.
- Create: `native/linux-combined-media-proof/src/container.rs`
- Create: `native/linux-combined-media-proof/src/progressive_guard.rs`
- Create: `native/linux-combined-media-proof/tests/combined_roundtrip.rs`

**Interfaces:**

```rust
pub struct CombinedMetrics {
    pub video: VideoMetrics,
    pub audio: AudioMetrics,
    pub video_duration_us: u64,
    pub coded_audio_duration_us: u64,
    pub av_end_delta_us: u64,
}

pub struct DemuxedMedia {
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
    pub video_samples: Vec<VideoAccessUnit>,
    pub audio_config: AacLcConfig,
    pub audio_samples: Vec<Vec<u8>>,
}

pub fn write_progressive_mp4(
    path: &Path,
    video: &EncodedVideo,
    audio: &EncodedAudio,
) -> Result<(), ContainerError>;

pub fn reopen_owned_progressive_mp4(path: &Path) -> Result<DemuxedMedia, ContainerError>;
pub fn run_combined_roundtrip(path: &Path) -> Result<CombinedMetrics, ProofError>;
```

`CombinedMetrics` contains only codec, content, and timing measurements. Rust does not compute or carry an MP4 cryptographic hash, and this task adds no Rust hashing implementation or hashing dependency; the approved `serde_json` build edge serves only strict manifest parsing. Task 5's Node verifier hashes the actual `combined.mp4` artifact after the proof exits.

`ProgressiveGuard::inspect(&mp4::Mp4Reader<_>)` requires a nonfragmented file, exactly one H.264 and one AAC track, no `edts/elst` entries, zero rendering offsets, increasing/in-range `stss`, `stts` totals equal to `stsz.sample_count`, expected sample sizes, and checked duration arithmetic. Release uses panic unwind, so demux catches the known `mp4` `read_sample` panic and returns `ContainerError::ParserPanic`; the guard does not calculate byte offsets or replace `Mp4Reader`.

- [x] **Step 1: Write container guard and mux/demux RED tests**

Add tests for: rejecting `is_fragmented`; rejecting a track edit; rejecting nonzero composition offsets; rejecting an `stts`/`stsz` mismatch before `read_sample`; rejecting unsorted/out-of-range `stss`; and accepting the exact facts produced by this proof. Use small internal `GuardFacts` values for branch coverage plus one real `Mp4Reader` assertion against the generated file so tests exercise the canonical crate tables.

```rust
#[test]
fn combined_mp4_reopens_and_decodes_owned_streams() {
    let output = NewTestPath::new("combined.mp4");
    let metrics = run_combined_roundtrip(output.path()).expect("combined proof");
    assert_eq!(metrics.video.decoded_frames, VIDEO_FRAME_COUNT);
    assert_eq!(metrics.video_duration_us, 1_000_000);
    assert!(metrics.audio.padding_frames <= MAX_CODED_PADDING_FRAMES);
    assert!(metrics.av_end_delta_us <= MAX_AV_END_DELTA_US);
}
```

- [x] **Step 2: Run the combined test and capture RED**

Run:

```bash
rtk mkdir -p "$PWD/output/linux-combined-media-proof/test-link-maps"

rtk env \
  VC_COMBINED_NATIVE_MANIFEST="$PWD/output/linux-combined-media-proof/native-build-attempt4/native-build.json" \
  VC_COMBINED_NATIVE_ROOT="$PWD/output/linux-combined-media-proof/native-build-attempt4" \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$PWD/native/linux-combined-media-proof/build-support/linker.sh" \
  VC_COMBINED_TOOLCHAIN_ROOT="$PWD/output/linux-combined-media-proof/inputs/toolchain" \
  VC_COMBINED_MUSL_ROOT="$PWD/output/linux-combined-media-proof/inputs/musl" \
  VC_COMBINED_LINK_MAP_DIR="$PWD/output/linux-combined-media-proof/test-link-maps" \
  RUSTC="$PWD/output/linux-combined-media-proof/inputs/rust/bin/rustc" \
  CARGO_HOME="$PWD/output/linux-combined-media-proof/inputs/cargo-home" \
  RUSTFLAGS="-C save-temps" \
  "$PWD/output/linux-combined-media-proof/inputs/rust/bin/cargo" test --manifest-path native/linux-combined-media-proof/Cargo.toml --locked --offline --features native-codecs --target x86_64-unknown-linux-musl combined_mp4_reopens_and_decodes_owned_streams -- --nocapture
```

Expected: FAIL because `container`, `progressive_guard`, and `run_combined_roundtrip` do not exist.

- [x] **Step 3: Implement MP4 writing through the real crate API**

Use `mp4::Mp4Writer::write_start`, `add_track`, `write_sample`, and `write_end`. Configure:

```rust
TrackConfig {
    track_type: TrackType::Video,
    timescale: VIDEO_TIMESCALE,
    language: "und".to_owned(),
    media_conf: MediaConfig::AvcConfig(AvcConfig {
        width: WIDTH as u16,
        height: HEIGHT as u16,
        seq_param_set: video.sps.clone(),
        pic_param_set: video.pps.clone(),
    }),
}
```

Write each video `Mp4Sample` at `start_time = index * 3_000`, `duration = 3_000`, `rendering_offset = 0`, with the bridge's sync flag and AVCC bytes. Configure AAC with `AacConfig { bitrate: 128_000, profile: AudioObjectType::AacLowComplexity, freq_index: SampleFreqIndex::Freq48000, chan_conf: ChannelConfig::Stereo }`, a 48,000 timescale, 1,024 duration, zero rendering offset, all audio samples sync, and raw AAC bytes. Write to a newly created file and remove a partial file on error.

- [x] **Step 4: Implement the narrow guard and real demux path**

Open with `mp4::Mp4Reader::read_header`; identify tracks using `Mp4Track::media_type`; inspect the public `trak.mdia.minf.stbl` tables; then use `Mp4Reader::read_sample` for bytes/timestamps. Read SPS/PPS from `avc1.avcc.sequence_parameter_sets[*].bytes` and `picture_parameter_sets[*].bytes`. Read AAC object type/frequency/channel using the track's `audio_profile`, `sample_freq_index`, and `channel_config` accessors. Require AOT 2, 48 kHz, stereo before calling the decoder.

Do not copy the prior sample-offset index or implement a custom demuxer. The guard exists to make `mp4` 0.14.0 honest for this generated progressive subset. Its unsupported errors must say that production common-MP4 support remains open.

- [x] **Step 5: Decode demuxed bytes and assert the fixed metrics**

Pass only data/config read back from the MP4 to the Task 3 decoders. Validate exact track/sample timing, then the video/audio fidelity rules. Calculate:

```rust
let video_duration_us = 30_u64 * 3_000 * 1_000_000 / 90_000;
let coded_audio_duration_us = audio_sample_count * 1_024 * 1_000_000 / 48_000;
let av_end_delta_us = video_duration_us.abs_diff(coded_audio_duration_us);
```

Require `video_duration_us == 1_000_000`, padding `coded_frames - 48_000 <= 1_023`, and `av_end_delta_us <= 21_334`. Retain actual decoded audio lag separately; do not subtract it to manufacture a gapless duration result.

- [x] **Step 6: Run guard and combined tests GREEN**

Run:

```bash
rtk mkdir -p "$PWD/output/linux-combined-media-proof/test-link-maps"

rtk env \
  VC_COMBINED_NATIVE_MANIFEST="$PWD/output/linux-combined-media-proof/native-build-attempt4/native-build.json" \
  VC_COMBINED_NATIVE_ROOT="$PWD/output/linux-combined-media-proof/native-build-attempt4" \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$PWD/native/linux-combined-media-proof/build-support/linker.sh" \
  VC_COMBINED_TOOLCHAIN_ROOT="$PWD/output/linux-combined-media-proof/inputs/toolchain" \
  VC_COMBINED_MUSL_ROOT="$PWD/output/linux-combined-media-proof/inputs/musl" \
  VC_COMBINED_LINK_MAP_DIR="$PWD/output/linux-combined-media-proof/test-link-maps" \
  RUSTC="$PWD/output/linux-combined-media-proof/inputs/rust/bin/rustc" \
  CARGO_HOME="$PWD/output/linux-combined-media-proof/inputs/cargo-home" \
  RUSTFLAGS="-C save-temps" \
  "$PWD/output/linux-combined-media-proof/inputs/rust/bin/cargo" test --manifest-path native/linux-combined-media-proof/Cargo.toml --locked --offline --features native-codecs --target x86_64-unknown-linux-musl -- --nocapture --test-threads=1
```

Expected: all fixture, negative-validator, direct-codec, guard, and combined tests PASS. The combined output is two tracks, 30 video samples, AAC-LC 48 kHz stereo samples of 1,024 frames, with actual content metrics inside the fixed limits.

- [x] **Step 7: Commit and pass Container Gate review**

```bash
rtk git add native/linux-combined-media-proof/src/lib.rs native/linux-combined-media-proof/src/container.rs native/linux-combined-media-proof/src/progressive_guard.rs native/linux-combined-media-proof/tests/combined_roundtrip.rs
rtk git commit -m "feat(linux): prove combined H264 AAC MP4 roundtrip"
```

Container Gate reviewer: a fresh Sol agent verifies the test feeds demuxed bytes/config to decoders, the writer/reader are real `mp4` APIs, timing units are consistent, all rejected features remain open production requirements, and same-library validation is disclosed.

### Task 5: Build the proof executable and capture provenance/runtime evidence

**Files:**
- Modify: `native/linux-combined-media-proof/build.rs` — emit one successful JSON receipt for the exact bridge compiler/archive commands and paths; add no dependency.
- Create: `native/linux-combined-media-proof/build-support/recording-rustc-wrapper.mjs`
- Create: `native/linux-combined-media-proof/build-support/recording-outer-linker.mjs`
- Create: `native/linux-combined-media-proof/src/main.rs`
- Create: `native/linux-combined-media-proof/README.md`
- Create: `scripts/build-linux-combined-media-proof.mjs`
- Create: `scripts/build-linux-combined-media-proof.test.ts`
- Create: `scripts/verify-linux-combined-media-proof.mjs`
- Create: `scripts/verify-linux-combined-media-proof.test.ts`

**Interfaces:**
- Build driver: `buildLinuxCombinedMediaProof({ preparedPath, nativeBuildPath, output, run, signal? }) -> Promise<BuildReceipt>` and CLI `node scripts/build-linux-combined-media-proof.mjs --prepared ABSOLUTE_JSON --native-build ABSOLUTE_JSON --output NEW_ABSOLUTE_ROOT`. It refuses a pre-existing output and owns `<output>/target`, `maps`, `logs`, `receipts`, `launchers`, and `tmp`. The CLI installs scoped `SIGINT`/`SIGTERM` handlers through one `AbortController`, restores them on exit, and propagates its signal through prepared-input revalidation and every owned child run so cancellation reaps the active child before rejection.
- Stable driver outputs are `<output>/build-receipt.json`, `<output>/binary-ref.json`, and `<output>/link-receipt.json`. `binary-ref.json` records the exact executable path reported by the successful final Cargo JSON artifact plus its SHA-256. `link-receipt.json` records the successful current outer-link invocation whose actual `-o` output has the same byte hash, plus that output, its real adjacent `.link-map-path` sidecar, selected map, and their hashes. `build-receipt.json` hashes/references both records, every raw log/receipt, the exact Cargo argv/environment, input manifests, and source inventories. No consumer guesses a target basename or sidecar location.
- Before invoking Cargo, strongly revalidate the prepared manifest and immutable native-build manifest, require the latter's hash and every declared built-artifact/runtime-input path and hash—including all eleven current native runtime inputs—to match regular accepted files beneath its native root, and pass the same optional signal through revalidation. Inventory in sorted path/hash order the crate `Cargo.toml`, `Cargo.lock`, `build.rs`, every Rust source, every native bridge/header/test source, `scripts/build-linux-combined-media-proof.mjs`, both recording wrappers, and the existing `build-support/linker.sh`. Reject any inventoried symlink or real path outside its expected checked-in root, and require the identical aggregate inventory after Cargo exits. Preserve `build-support/linker.sh` byte-for-byte at SHA-256 `9a3d259dd37df549ef46428fa8ead75e4bd56e1844a5059bacaeae62d4bf808a`; the outer linker delegates to it and does not replace its reviewed CRT/link behavior.
- The driver resolves and records the actual Node executable path/version/hash, then generates `<output>/launchers/rustc-wrapper` and `<output>/launchers/outer-linker`, each invoking that absolute Node binary and its absolute checked-in wrapper path. `recording-rustc-wrapper.mjs` requires Cargo's first wrapper argument to equal the recorded pinned rustc, preserves the remaining invocation as an argv array without string joining, classifies it as host or `x86_64-unknown-linux-musl` target from `--target VALUE` or `--target=VALUE`, and rejects another target. After success it records the output directory, crate name, extra filename, strict generated `outputs`, and pre-build source-inventory hash under `<output>/receipts/rustc`; host and target receipts remain distinct. A separate typed `observedEphemeralOutputs` entry may use reason `rustc-llvm-ir-output-alias` only for the exact `<crate-name><extra-filename>.ll` path when that invocation requests `llvm-ir` in `--emit` and a same-invocation, same-output-directory `<crate-name><extra-filename>.*.rcgu.ll` was retained with the identical hash. At validation, an existing alias must remain a regular nonsymlink with that hash; `ENOENT` is permitted only after revalidating all alias metadata and the retained sibling's current hash. Every `.rcgu.o` and ordinary output remains strict. Reject every other missing, malformed, escaping, or mismatched output; do not copy IR files or broaden the exception. `recording-outer-linker.mjs` records the original linker argv, verifies the immutable legacy-linker hash, delegates to that exact script, and only after success records the actual absolute `-o`, adjacent sidecar, sidecar-selected map, and all three hashes under `<output>/receipts/link`.
- With `native-codecs`, `build.rs` enters receipt mode when any of `VC_COMBINED_RECEIPT_ROOT`, `VC_COMBINED_NATIVE_MANIFEST_SHA256`, or `VC_COMBINED_SOURCE_INVENTORY_SHA256` is present and validates all three before spawning any compiler or archive command. The Node driver validates the native manifest and source inventory before supplying their hashes. Receipt mode writes `<OUT_DIR>/vc-codec-bridge-command-receipt.json` only after both compilers and `ar` succeed, using existing `serde_json` to record the exact clang++, clang, and ar executable paths/argv arrays, source paths, object paths, archive path, and supplied hashes. Partial recording values fail before commands; ordinary native-codec tests with none preserve existing behavior and emit no qualifying receipt. Rust performs no cryptographic hashing. After Cargo exits, the Node driver requires every bridge source beneath the checked-in native source root and every object/archive/receipt beneath the expected real `OUT_DIR`, rejects symlinks and escaping paths, then hashes those files.
- Proof CLI: `linux-combined-media-proof <new-output-directory>`. It creates the directory, writes `combined.mp4` and `metrics.txt`, prints one `COMBINED_MEDIA_PASS` line, and returns nonzero without overwriting any existing path.
- Verifier CLI: `node scripts/verify-linux-combined-media-proof.mjs --build-receipt ABSOLUTE_JSON --prepared ABSOLUTE_JSON --native-build ABSOLUTE_JSON --output NEW_DIRECTORY --readelf ABSOLUTE_FILE --nm ABSOLUTE_FILE --file ABSOLUTE_FILE --strace ABSOLUTE_FILE`. `--build-receipt` is mandatory and is the sole binary/link-map discovery input; remove separate binary, link-map record, and map-root options. Resolve external inspection-tool aliases before passing them: every tool path must name the actual absolute regular executable rather than a symlink. The verifier records each actual binary's path, version, and SHA-256.
- `verifyCombinedMediaProof(options) -> Promise<VerificationSummary>` owns/reaps only children it starts, applies a 120-second runtime timeout, and writes `summary.json` only after all checks pass.

Task 5a is complete after typed-alias fix `3873ee08`, symlink fix/review `ec0f88c8`, and accepted fresh attempt 2. Attempt 2's build-receipt SHA-256 is `6111941c0187f5ee9c070d14080dfe86176efc7c14827588c07652d1b31cbcd7`, binary SHA-256 is `ae24339fa592624d2f3ea4fb4486bed8601a48beadd39762e1cabdcbfa9a0d7a`, and map SHA-256 is `886ff7f5f7902ba0e2fd4f0d8ce0c4c6d489e82c44b149bdc95a972ae57b33d2`. Task 5b is complete at `b1a50de` with accepted evidence attempt 13 and a Fresh Evidence Gate PASS. Independent review passed `docs/research/2026-09-12-linux-getrandom-weak-probe.md` at `ea28d77d` for this exact binary only. Task 5 is complete; the earlier attempt-7 through attempt-12 verifier states remain preserved as review-incomplete, superseded history.

#### Task 5a: Build driver and receipts

- [x] **Step 1: Write build-driver and wrapper RED tests**

Use fake Cargo/rustc/legacy-linker processes and injected filesystem fixtures. Prove refusal of a reused output, a changed legacy-wrapper hash, an unsorted/incomplete source inventory, any pre/post source change, a Cargo artifact marked `fresh`, a selected executable lacking a successful current link receipt with an equal byte hash, an `.rcgu.o` without one matching successful target-rustc argv receipt/source hash, and bridge objects/archive without the exact successful build-script command receipt. Prove the driver accepts a legacy sidecar beside a hashed `deps/` link output even when Cargo reports a different top-level executable path with identical bytes; it must not look for a top-level sidecar.

Wrapper tests must prove host and target rustc invocations remain distinct and argv arrays preserve argument boundaries, spaces, empty strings, and literal shell metacharacters. Outer-linker tests cover `-o FILE` and `-oFILE`, require the actual output-local sidecar/map, and prove no successful receipt is published when the delegated legacy linker fails. Add regressions for a native artifact whose path/hash disagrees with its manifest, each partial combination of the three receipt-mode variables before any command runs, aborting/reaping an active revalidation or child command on `SIGINT`/`SIGTERM`, a symlink or root escape in the checked-in source and generated bridge inputs/outputs, and exact recorded `CARGO_BUILD_JOBS=2` rather than an inherited/default job count. For the observed LLVM-IR alias, prove only the exact typed alias may disappear; reject absent or changed retained siblings, missing `.rcgu.o` or ordinary outputs, aliases without `--emit=llvm-ir`, wrong crate/extra filename/output directory, symlinks, and changed aliases that still exist.

- [x] **Step 2: Run Task 5a tests and capture RED**

Run: `rtk node --test scripts/build-linux-combined-media-proof.test.ts`

Expected: FAIL because the build driver and recording wrappers do not exist and `build.rs` emits no bridge command receipt.

- [x] **Step 3: Implement the fresh build driver and recording contracts**

After the strong revalidation above, construct Cargo's child environment from only the recorded values required for this build: `CARGO_HOME`, `CARGO_NET_OFFLINE=true`, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`, fresh `CARGO_TARGET_DIR`, `CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER` pointing to the generated outer-linker launcher, absolute `RUSTC`, absolute output-local `RUSTC_WRAPPER` launcher, `RUSTFLAGS=-C save-temps`, `VC_COMBINED_NATIVE_MANIFEST`, its validated `VC_COMBINED_NATIVE_MANIFEST_SHA256`, `VC_COMBINED_NATIVE_ROOT`, `VC_COMBINED_TOOLCHAIN_ROOT`, `VC_COMBINED_MUSL_ROOT`, `VC_COMBINED_LINK_MAP_DIR`, `VC_COMBINED_RECEIPT_ROOT`, `VC_COMBINED_SOURCE_INVENTORY_SHA256`, `VC_COMBINED_LEGACY_LINKER`, `LC_ALL=C`, `TZ=UTC`, fixed `/usr/bin:/bin` development `PATH`, and output-local `TMPDIR`. Do not inherit plugin/codec/library override variables. Propagate `signal` to every `runOwnedProcess` call, record the exact environment including the two-job cap, and record the exact ordered Cargo argv:

```text
build --manifest-path <absolute Cargo.toml> --locked --offline --release --features native-codecs --target x86_64-unknown-linux-musl --bin linux-combined-media-proof --message-format=json-render-diagnostics
```

Retain raw stdout/stderr and every parsed Cargo JSON message. Require exactly one successful non-fresh executable artifact for `linux-combined-media-proof`. A new output/target/map root and a current successful link receipt are mandatory, so cached Cargo `Fresh` output can never reuse an old map. Select the link receipt by equal SHA-256 between Cargo's reported executable and the actual hashed link output, then write `binary-ref.json`, `link-receipt.json`, and finally `build-receipt.json` atomically. Keep the accepted prepared/native inventories as prior evidence and do not rebuild Task 2 archives.

For every saved `.rcgu.o`, require exactly one successful target-rustc receipt whose exact argv, output directory, crate name, extra filename, recorded output path/hash, and source-inventory hash produced and bind it. Reject ambiguous, unmatched, or multiply claimed outputs. Require the build-script receipt to bind every generated bridge object/archive to exact clang/clang++/ar commands and the same source hash. Retain host rustc receipts for build scripts/proc macros separately; never use them to classify target objects.

- [x] **Step 4: Run Task 5a tests GREEN without a final build**

Run: `rtk node --test scripts/build-linux-combined-media-proof.test.ts`

Expected: PASS for injected driver/wrapper/receipt cases only. Do not invoke the actual driver before Container Gate passes.

Build Receipt Gate passed independently at `368d2d63`; job-cap follow-up `37964740` passed scoped review; typed-alias fix `3873ee08` passed 61 tests; and symlink hardening `ec0f88c8` passed 62 tests in review. Fresh attempt 2 produced the accepted build receipt, binary, and map hashes recorded above. Its retained inner Cargo stderr contains 40 jobserver-FD warnings: retain them as a development-log limitation. `CARGO_BUILD_JOBS=2` proves the requested Cargo scheduling cap, not a measured cap on every nested build-tool thread. Task 5 completed after the attempt-13 verifier and Fresh Evidence Gate PASS.

- [x] **Step 5: Commit Task 5a and pass Build Receipt Gate review**

```bash
rtk git add native/linux-combined-media-proof/build.rs native/linux-combined-media-proof/build-support/recording-rustc-wrapper.mjs native/linux-combined-media-proof/build-support/recording-outer-linker.mjs scripts/build-linux-combined-media-proof.mjs scripts/build-linux-combined-media-proof.test.ts
rtk git commit -m "build(linux): record combined proof build provenance"
```

Build Receipt Gate reviewer: a fresh Sol agent verifies the legacy-linker bytes/hash remain unchanged, launcher Node paths are absolute and recorded, receipts preserve argv arrays, the source inventory is complete/stable, bridge and `.rcgu.o` bindings are exact, and fake cached/mismatched artifacts fail. This is development-build evidence only and changes no source/license policy.

#### Task 5b: Proof CLI and verifier

- [x] **Step 6: Write proof CLI and verifier RED tests**

CLI tests cover missing/extra options, pre-existing-directory refusal with an untouched sentinel, and successful exact filenames. Verifier fixtures prove mandatory build-receipt validation and rejection of a changed binary/map/sidecar/receipt hash, missing or mismatched rustc/bridge receipts, `DT_NEEDED`, an ELF interpreter, a link-map contributor absent from the prior prepared/native/Cargo inventories, a forbidden GNU runtime contributor, an arbitrary Cargo-lock drift/derivation, and any unresolved weak symbol outside exactly `__cxa_finalize`, `__cxa_thread_atexit_impl`, `__deregister_frame_info`, `__register_frame_info`, and `gettid`, except the exact audited `getrandom` predicate in Step 9. Test every predicate conjunct independently and prove an otherwise identical unqualified `getrandom` remains rejected. Always reject `pidfd_getpid` and `pidfd_spawnp`.

Archive fixtures accept a generated Rust aggregate only through the receipt-bound rule in Step 9. Cover unique members; duplicate names with identical or differing hashes from one approved native archive; and an exact ordered concatenation from multiple approved archives with the same source-lock identity, revision, and license origin. Include the actual `impd_drc_api.c.o` pattern with its encoder candidate followed by its decoder candidate. Apply the complete ordered-candidate rule to directly linked approved archives too: cover musl `libc.a(realloc.lo)` with its two different candidates, and reject zero candidates, a changed parent-archive hash, or unknown candidate bytes. Reject any unknown, extra, missing, reordered, unmatched, or first-match-only candidate; a concatenation unsupported by retained rustc/Cargo link-order evidence; conflicting origins; and multiple non-equivalent matches.

Weak-probe fixtures accept a final `nm` weak-undefined name even when LLD has no row for the undefined symbol, but only when raw `nm -A --undefined-only` evidence identifies a qualified referencing object/member that has an exact contributing map row and matching artifact/member hash. Cover the three `crtbeginS.o` probes and direct `.rcgu.o` `gettid` reference described in Step 9. Reject references from noncontributing members, missing/mismatched map rows or hashes, an unqualified source, and any claim that the absent `__cxa_thread_atexit_impl` was mapped.

The CLI-test portion is independently approved at `2dc5c0ab`; verifier fixtures pass at `7a9135c3`.

Trace fixtures cover plain and optional `[pid N]` prefixes, single-line and paired `<unfinished ...>`/`<... clone3 resumed>` records, interleaving by PID, unmatched/malformed resumed calls, every additional `execve`, `fork`, `vfork`, and process-creating/unknown `clone`/`clone3`. Permit only a fully reconstructed observed clone whose explicit flags include `CLONE_THREAD`; never infer behavior from source.

Receipt-inventory fixtures pin this exact Rust 1.97.1, 22-registry-package proof graph. The positive fixture is accepted final-build attempt 2. Negative fixtures coherently delete a host compile receipt or target compile receipt together with its Cargo compiler-artifact event, raw-evidence entry, and every forward reference; delete each class of special query receipt; and introduce duplicate, unmatched, or wrong-form compile/query receipts. Add the actual two-variant `serde_core` custom-build case: copy one surviving host variant to a new JSON path and coherently rewrite its Cargo event, receipt, and raw references to impersonate the deleted variant, including a full survivor-copy mutation of the host receipt metadata; it must fail. Add host and target negatives for duplicate identity options in both split and combined rustc argv forms, including conflicting `extra-filename` values; every logical identity option must occur exactly once and every negative must fail. The production verifier must use the same fixed expected graph and query forms; fixtures or a caller-supplied graph cannot bypass that inventory.

```js
test("rejects a proof executable that starts a helper", async () => {
  const trace = `100 execve("/abs/linux-combined-media-proof", ...) = 0\n` +
                `100 execve("/usr/bin/ffmpeg", ...) = 0\n`;
  await assert.rejects(
    verifyFixture({ processTrace: trace }),
    /proof executable started another process.*ffmpeg/,
  );
});

test("allows a resumed explicitly traced same-process thread", async () => {
  const trace = `execve("/abs/linux-combined-media-proof", ...) = 0\n` +
                `[pid 100] clone3({flags=CLONE_VM|CLONE_THREAD, <unfinished ...>\n` +
                `[pid 100] <... clone3 resumed> child_tid=...}, 88) = 101\n`;
  await assert.doesNotReject(verifyFixture({ processTrace: trace }));
});

test("rejects clone without an explicit thread flag", async () => {
  const trace = `100 execve("/abs/linux-combined-media-proof", ...) = 0\n` +
                `100 clone(child_stack=..., flags=CLONE_VM|SIGCHLD, ...) = 101\n`;
  await assert.rejects(
    verifyFixture({ processTrace: trace }),
    /process-creating or unknown clone flags/,
  );
});
```

- [x] **Step 7: Run Task 5b tests and capture RED**

Run: `rtk env RUSTC="$PWD/output/linux-combined-media-proof/inputs/rust/bin/rustc" CARGO_HOME="$PWD/output/linux-combined-media-proof/inputs/cargo-home" "$PWD/output/linux-combined-media-proof/inputs/rust/bin/cargo" test --manifest-path native/linux-combined-media-proof/Cargo.toml --locked --offline --no-default-features cli_`

Run: `rtk node --test scripts/verify-linux-combined-media-proof.test.ts`

Recorded result: the CLI and verifier RED cases prove the intended missing behavior; their final GREEN runs are recorded below.

- [x] **Step 8: Implement the runtime CLI**

Parse exactly one non-option path, create it with `create_dir`, call `run_combined_roundtrip`, write stable `key=value` metrics including every fixed threshold and actual measurement, sync the files, and print success. Do not use `std::process`, dynamic loading, environment-based codec discovery, or library/plugin search variables.

Independent CLI review passed four pure tests, four native tests, and a real retained debug smoke producing two files, 30 metric keys, and one success marker. Argument and overwrite refusal passed, including an unchanged artifact hash on rerun. This is CLI behavior evidence only; it makes no final-ELF or receipt claim.

- [x] **Step 9: Implement receipt-bound evidence capture and contributor classification**

The verifier must:

1. Refuse non-absolute input/tool/receipt paths, every symlink including an inspection-tool alias, and pre-existing output. Require callers to resolve aliases first, then validate and record the actual regular tool binaries.
2. Validate `build-receipt.json` and every referenced hash/path beneath its fresh build root. Obtain the exact Cargo-reported executable and selected map only through `binary-ref.json` and `link-receipt.json`; require their executable/link-output byte hashes to match and reject any separately guessed basename or sidecar.
3. Run that exact proof binary under absolute `strace` with `PATH` empty, `LC_ALL=C`, `TZ=UTC`, and codec/plugin/library override variables removed.
4. Parse observed process tracing without source inference. Normalize optional `[pid N]` prefixes and pair unfinished/resumed calls by PID plus syscall before policy checks. Require exactly the verifier's initial proof `execve`; reject malformed/unmatched records, every additional `execve`, every `fork`/`vfork`, and every `clone`/`clone3` unless the reconstructed flags explicitly include `CLONE_THREAD`. Numeric, missing, or unknown flags fail.
5. After the proof exits, use built-in `node:crypto` to hash the actual generated `combined.mp4` and every retained artifact. Retain stdout/stderr, process trace, MP4/metrics, `file`, `readelf -hW/-lW/-dW`, and `nm -u` output.
6. Require x86-64 ELF `DYN` static PIE, no interpreter, no `NEEDED`, and no strong unresolved symbol. From final `nm` output, the general allowlist remains a subset of exactly `__cxa_finalize`, `__cxa_thread_atexit_impl`, `__deregister_frame_info`, `__register_frame_info`, and `gettid`, each precisely weak undefined. LLD need not print an undefined-name row. Instead, for every observed probe, retain raw `nm -A --undefined-only` evidence for at least one referencing object/member and require that exact object/member to contribute through a matching map row, artifact/member hash, and receipt, approved native manifest, or eligible upstream-source qualification. A noncontributing member cannot qualify a probe. In the accepted binary, `__cxa_finalize`, `__deregister_frame_info`, and `__register_frame_info` are referenced by accepted `crtbeginS.o` with hash beginning `4a18df51…` and six matching map rows; `gettid` is referenced by the selected receipted `.rcgu.o` with hash beginning `4f19f742…` and a direct map row. `__cxa_thread_atexit_impl` is absent, so do not claim it was mapped. Permit weak undefined `getrandom` only when every part of this audited executable predicate holds together:

   - the receipt-bound binary SHA-256 is exactly `ae24339fa592624d2f3ea4fb4486bed8601a48beadd39762e1cabdcbfa9a0d7a`, with a valid fresh build receipt and unchanged source inventory;
   - Rust is pinned to commit `8bab26f4f68e0e26f0bb7960be334d5b520ea452`, official `rust-std-1.97.1-x86_64-unknown-linux-musl.tar.xz` hashes to `51d83178680556f73a5fa8ad865b76a1ff541867445c00fc65dc67246bc2de66`, contributing `libstd` hashes to `8ff60d366af7bc26c8808031a1384bb3a681ee9a65f44cb35a655e7e4cfe143c`, and contributing musl `libc.a` hashes to `657e951f4c1d02ecac4e424222034c84a189f361b090f1284f255b3ac4480d5c`;
   - the selected map contains `syscall.lo` and does not contain `getrandom.lo`;
   - `getrandom` is precisely weak undefined, `.dynsym` contains only its null entry, there is no dynamic relocation for `getrandom`, and the ELF has no interpreter or `NEEDED` entry; and
   - retained evidence contains no `dlopen`, `dlsym`, `dlclose`, or `dlerror` reference.

   The exact binary hash binds the independently audited zero-GOT, null-check, and raw-syscall machine code, so the verifier does not add a fragile general disassembler or hard-coded-address parser. Any changed binary or failed conjunct rejects `getrandom` and requires a new audit. Always reject `pidfd_getpid`, `pidfd_spawnp`, and every other unresolved symbol. This is an exact executable qualification, not a general name allowlist, LGPL/GNU exception, or policy change.
7. Parse the selected LLD map into unique contributors. Bind each generated `.rcgu.o` to its exact successful target-rustc argv receipt and unchanged source-inventory hash. Bind bridge objects and `libvc_codec_bridge.a` to the build-script clang/clang++/ar receipt. A rustc temporary aggregate such as `release/deps/rustcMyBdl2/liblinux_combined_media_proof-53fbee79e3c0941a.rlib` is generated evidence, not an upstream archive: require its canonical path to be a regular nonsymlink beneath the fresh target, its exact path in the successful final-link argv tied to the successful final target-rustc receipt/source hash, and its retained actual hash. Qualify every contributing aggregate member through an approved native archive, receipted bridge output, or eligible upstream archive member. The current aggregate hash beginning `3b83af…` differs from the canonical crate rlib beginning `7e579b…`; never treat it as a same-hash alias. The exact rustc creation invocation plus final-link consumption and qualified member bytes are sufficient; no separate internal Rust `ar` command is required.

   For a contributing member name duplicated inside the aggregate, record every candidate's parent archive plus the complete candidate hash sequence in archive order with multiplicity. Accept either an exact match to one approved native archive or a complete ordered concatenation of approved archives that share the exact source-lock identity, revision, and license origin. Bind concatenation order to the observed target-rustc `-l` arguments or retained Cargo `build-script-executed` `linked_libs` plus `linked_paths`; do not infer it from filenames. The accepted order is `[vc_codec_bridge, openh264, xaacenc, xaacdec, c++abi, unwind]`, all static, with native search path `native-build-attempt4/lib`. The actual `impd_drc_api.c.o` sequence contains one `libxaacenc` candidate followed by one `libxaacdec` candidate, both from the exact same libxaac source identity/revision/license origin. The final raw link argv consumes only the aggregate, so do not claim it directly names those native archives.

   The current aggregate has 17 duplicated member names: 13 have identical candidate hashes and four OpenH264 names have different candidate hashes. Because the LLD map does not identify which occurrence it selected, retain every candidate and explicitly report occurrence selection as ambiguous; do not claim a chosen member. Reject unknown, extra, missing, reordered, or unmatched candidates, conflicting source identity/revision/license origin, an unrecorded concatenation, multiple non-equivalent matches, and every first-match shortcut. Classify all other Cargo/native contributors only through prior manifests, verified Rust archive membership, current Cargo lock/checksums/licenses, and exact member hashes. This refines origin qualification and creates no LGPL/GNU exception.

   Apply the same complete ordered-candidate rule when the map directly names an approved archive with duplicate member names. No cross-archive inference is needed: require the exact retained parent-archive hash and match every candidate byte sequence in that archive. Accepted musl `libc.a(realloc.lo)` has two candidates in order—size 2,872 with hash beginning `9176cf75…`, then size 1,032 with hash beginning `84061f82…`—both from the same pinned musl-dev 1.2.4-2 and license origin. Record both sizes/hashes and occurrence ambiguity without claiming one chosen hash. Reject zero candidates, changed archive bytes/hash, unknown candidate bytes, missing/reordered candidates, or any selected-occurrence claim.
8. Explicitly reject GNU CRT, `libgcc`, `libstdc++`, glibc, replaced rustup CRT hashes, paths outside approved prepared/native/fresh-build/Cargo roots, unclassified inputs, changed receipt inputs, and any pre/post source-inventory difference.
9. Accept either a full fresh prepared snapshot from the latest lock or the exact reviewed Task 3 Cargo-only derivation. For the retained derivation, require base SHA-256 `ed1be347b5d8155ca5d41002973db948f6a9ea574352473a5a537d331f86e170`, approved old/new lock hashes, unchanged source/toolchain/musl/rlib/22-package inventories, and immutable attempt-4 native manifest SHA-256 `ce89fb8f9fe9bc5584846db14cf7d294c83983242157eebf5af9efef335f263e`. Reject other derivations. A matching full-fresh prepared/native pair needs no exception.
10. Copy the checked-in notice set, Cargo metadata, build/source inventories, every exact command/receipt/raw log, source revisions, archive hashes, and development-tool versions into evidence. Label Node, Cargo, rustc, wrappers, compilers, ar, ELF tools, and strace as development-only. Source-license snapshots remain source-level evidence, not a complete per-file obligation audit.
11. Authenticate receipt command semantics rather than accepting coherent receipt rewrites. Require the pinned Cargo executable, exact build argv, repository cwd, complete expected environment, and both exact recording launchers. For every host and target compile rustc receipt, unconditionally require the pinned rustc executable and bind the metadata target/classification, crate name, output directory, extra filename, emitted output contract, and source-inventory identity to the exact argv and outputs. Require exactly one occurrence of every logical rustc identity option across both split and combined argv spellings; never accept the first of duplicate or conflicting values. Target-only validation is insufficient. For the bridge receipt, require the exact pinned compiler and archiver executables, argv order, flags, input paths, output paths, and archive-member contract. Add negative fixtures that rewrite a receipt and all of its internal hashes coherently while changing one required semantic; every such fixture must fail.
12. Make the verifier itself part of the evidence boundary before writing `PASS`. Record and copy its exact checked-in entry source with canonical absolute path and SHA-256; record the exact canonical CLI/options argv; record the executing Node absolute regular path, version, and SHA-256; include every imported validation helper identity not already covered by the accepted build inventory; and require the verifier entry plus imported self-source inventory to match before and after verification. Include these records in artifact-manifest coverage before `summary.json` may report `PASS`. This is local provenance and stability evidence, not a signed attestation or a claim about who invoked the verifier.
13. Prove rustc-receipt completeness from an independent, fixed expected multiset for this exact Rust 1.97.1, 22-registry-package proof graph, never from the surviving receipt files or observed Cargo events. Build the expected 39 compiler-artifact multiset from the verified 22 registry package records plus the checked-in project targets and build script. Each expected entry pins package, version, host/target classification, artifact kind, name, exact source path, exact Cargo feature set, exact Cargo profile, coherent artifact filenames/output identity, and multiplicity. The two `serde_core` custom-build variants are distinct: one has features `["result", "std"]` and output identity beginning `5253…`; the other has features `["std"]` and output identity beginning `4f1d…`. Require the observed Cargo `compiler-artifact` multiset to match exactly. Then pair those 39 events one-to-one with the 39 compile rustc receipts—23 host and 16 target—using source path, crate name, classification, Cargo features/profile, rustc `--cfg feature` set, codegen profile, extra filename, output directory, artifact filenames, and emitted outputs. Enforce those metadata-to-argv/output bindings unconditionally for host and target receipts, with exact-one cardinality for each logical identity option across split and combined forms. Consume every logical invocation, event, and receipt exactly once and reject a duplicate logical invocation even when it uses a different receipt JSON path; count or path uniqueness alone is insufficient. Bind the final binary through the stable link hash.

    Separately require exactly seven special rustc receipts with exact full argv forms and multiplicities: host `--version --verbose` once; host `-vV` twice; the pinned Cargo capability query once for host and once for the musl target; and the two exact target autocfg stdin probes, `stdin_0` and `stdin_1`, once each. The aggregate assertions remain exactly 27 host plus 19 target equals 46 receipts, but those totals supplement rather than replace multiset, one-to-one, and query-form validation. Production accepts no fixture or custom-graph bypass. This rule proves only the fixed 22-package Rust 1.97.1 graph; any Rust, Cargo, query-form, package, target, or build-script graph change requires plan review and a newly pinned inventory. It does not change runtime, dependency, or source-license policy.

This gate closes the specific twelve-rustup-rlib distribution-origin gap only if the contributing `.rlib` hashes are members of the verified official `rust-std-1.97.1-x86_64-unknown-linux-musl.tar.xz` and that archive's manifest commit/license mapping passes review. Archive membership does not establish a complete per-file source-obligation audit. A matching installed-toolchain filename is insufficient.

- [x] **Step 10: After Container Gate PASS, run tests GREEN and build/verify once**

For the currently retained evidence, use the strongly revalidated Task 3 prepared snapshot with the immutable accepted Task 2 attempt-4 artifacts. The Cargo-only derivation preserves the approved native/source inventories and does not rebuild native code. A later full fresh run from the latest lock should use its directly prepared snapshot and matching fresh native build without this derivation:

```bash
rtk node --test scripts/build-linux-combined-media-proof.test.ts scripts/verify-linux-combined-media-proof.test.ts

rtk node scripts/build-linux-combined-media-proof.mjs \
  --prepared "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs-task3.json" \
  --native-build "$PWD/output/linux-combined-media-proof/native-build-attempt4/native-build.json" \
  --output "$PWD/output/linux-combined-media-proof/final-build"

rtk readlink -f /usr/bin/nm
rtk readlink -f /usr/bin/readelf

rtk node scripts/verify-linux-combined-media-proof.mjs \
  --build-receipt "$PWD/output/linux-combined-media-proof/final-build/build-receipt.json" \
  --prepared "$PWD/output/linux-combined-media-proof/inputs/prepared-inputs-task3.json" \
  --native-build "$PWD/output/linux-combined-media-proof/native-build-attempt4/native-build.json" \
  --output "$PWD/output/linux-combined-media-proof/evidence" \
  --readelf /usr/bin/x86_64-linux-gnu-readelf --nm /usr/bin/x86_64-linux-gnu-nm --file /usr/bin/file --strace /usr/bin/strace
```

The two `readlink` commands must print `/usr/bin/x86_64-linux-gnu-nm` and `/usr/bin/x86_64-linux-gnu-readelf` before the shown verifier invocation. If this host resolves different absolute regular binaries, pass those resolved paths and retain their actual path/version/hash instead; do not install a new tool solely to match the example.

Expected: tests PASS; the driver creates only the three stable top-level JSON outputs plus owned subdirectories and prints `COMBINED_MEDIA_BUILD_PASS`; verifier prints `COMBINED_MEDIA_VERIFY_PASS`. `summary.json` records the exact Cargo artifact/link-output byte match, source/command/receipt bindings, no runtime subprocess, no dynamic dependency, no forbidden/unclassified contributor, two MP4 streams, and all fixed content/timing tolerances. If `/usr/bin/strace` is unavailable, mark process-trace verification blocked; do not infer it from source.

Recorded final Task 5b evidence: fix `b1a50de` enforces the independent 39-event/compile-receipt multiset, seven exact special forms, unconditional host/target metadata-to-argv binding, and exact-one output-identity option cardinality. Evidence attempt 13 passes with summary SHA-256 `0b697df7b55f1e65b54e9fece746022d479a33d841b6619c4be88edb63f0bea9`, artifact-manifest SHA-256 `6d3cbea7e262d91cc743a032dc111c1f0f5e453db05c20c063f9e0c6e0cc84b2`, verifier-identity SHA-256 `cf25f318d8e4c1c6a2e3467195f2d9f70bfa4ed2aff9d9eb8ad141bab00c0194`, and verifier-entry SHA-256 `f6abbfab00a1ef8cc9d1f7f1cdc9e3cd99d65c8249f29354c91c6e8e3dbc0a75`. Its 265 records cover all 262 eligible files among 264 total files with zero mismatches. The focused suite passed 206 Node tests—144 verifier and 62 driver tests—and all four CLI tests. Fresh Evidence Gate independently passed `b1a50de` and attempt 13 with no findings or blocked checks inside the bounded Task 5b scope. Preserve failed attempts 1, 2, 3, and 5; attempts 4 and 6 are superseded passing checkpoints; attempts 7 through 12 passed their then-current checks but are review-incomplete and superseded. The accepted build-2 binary, MP4, media metrics, ELF, process trace, exact weak-probe predicates, and 596 contributor classifications remain unchanged.

- [x] **Step 11: Document operation and limitations**

README commands must use caller-specific absolute roots, explain the large development-only download/build, and list the exact guarded subset. State that successful same-library round trips do not establish independent decoding, generic H.264/AAC/MP4 conformance, edit/fragment/B-frame/VFR/rotation/seek behavior, production performance, patent clearance, packaging, or release readiness.

- [x] **Step 12: Commit Task 5b and pass Evidence Gate review**

```bash
rtk git add native/linux-combined-media-proof/src/main.rs native/linux-combined-media-proof/README.md scripts/verify-linux-combined-media-proof.mjs scripts/verify-linux-combined-media-proof.test.ts
rtk git commit -m "test(linux): verify combined media proof evidence"
```

Evidence Gate reviewer: a fresh Sol agent independently checks raw test/run output, MP4 metrics, ELF/link-map classifications, the Cargo-executable/link-output byte-hash join, Rust and bridge receipts against the unchanged source inventory, verified Rust archive membership, notices, and process trace. The reviewer reports passed, failed, blocked, and unverified checks separately.

### Task 6: Record the bounded result and obtain final independent review

**Files:**
- Create: `docs/research/2026-09-12-linux-combined-media-proof.md`

**Interfaces:**
- The report cites source commits/hashes, exact commands, raw evidence paths, actual metric values, link contributors, and all remaining gates. It changes no policy or production readiness field.

Fresh Evidence Gate passed for `b1a50de` and evidence attempt 13. The final report uses attempt 13 and retains the bounded verdict and exclusions. Its separate fresh report review also passed as recorded in Step 4; Task 6 is complete at the bounded decision gate.

- [x] **Step 1: Write the report from raw evidence**

Lead with one of three exact verdict shapes:

- `combined owned-fixture proof passed; production media backend remains unqualified`
- `combined owned-fixture proof failed:` followed by the exact functional or provenance check
- `combined owned-fixture proof blocked:` followed by the exact environment or input condition

Include tables for source/toolchain provenance, normal Cargo dependencies/licenses, final contributing artifacts, video fidelity, audio length/lag/correlation/RMS/padding, MP4 track timing, ELF/runtime-process results, and scope exclusions. State explicitly that codec and container validation is same-library and that no generic conformance is claimed.

- [x] **Step 2: Run final cheap checks**

```bash
rtk node --test scripts/prepare-linux-combined-media-proof.test.ts scripts/build-linux-combined-media-codecs.test.ts scripts/build-linux-combined-media-proof.test.ts scripts/verify-linux-combined-media-proof.test.ts
rtk git diff --check
rtk git status --short
```

Expected: all applicable tests PASS, diff check is clean, and status contains only the authorized report and plan-status changes before commit. Do not rerun expensive native source builds unless implementation changed their inputs, flags, or bridge code after the Evidence Gate run.

Recorded: all 268 tests passed across the four scripts at `b1a50de`; `git diff --check` passed. No expensive native source build was repeated.

- [x] **Step 3: Commit the evidence-backed report**

```bash
rtk git add docs/research/2026-09-12-linux-combined-media-proof.md docs/superpowers/plans/2026-09-12-linux-combined-media-proof.md
rtk git commit -m "docs: record Linux combined media proof"
```

- [x] **Step 4: Pass final independent review and stop at the decision gate**

A fresh Sol reviewer compares the report to the checked-in plan/spec and raw evidence, with special attention to the official Rust archive mapping, direct AAC API use, AVCC/Annex-B conversion, audio lag versus coded-duration reporting, progressive guard scope, and same-library disclosure. Resolve findings with focused commits and repeat only affected tests/review gates.

Recorded: fresh review of `b1a50de` through report commit `298286ef` passed with no findings. The reviewer independently confirmed all 268 final cheap tests, attempt-13 hash coverage, and clean status. Task 6 is complete at the bounded decision gate.

After approval, report this subproject's narrow result to root. Do not author the production backend-routing plan until root has reviewed this combined proof alongside the accepted prerequisite reviews. Even a full pass authorizes no UI rewrite, production backend selection, license-policy change, packaging claim, patent conclusion, or Linux release claim.
