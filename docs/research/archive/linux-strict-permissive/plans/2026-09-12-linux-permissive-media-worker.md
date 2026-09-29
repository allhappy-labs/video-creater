# Linux Permissive Media Worker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a generalized, bounded, static-musl worker that implements the v1 `Capabilities` and `EncodeMp4` operations over raw RGBA8 and optional 48 kHz stereo S16LE spools using direct OpenH264/libxaac APIs and an owned progressive MP4 writer.

**Architecture:** A standalone worker crate consumes the protocol crate without joining or changing the app workspace. Stateful native bridges return one encoded access unit at a time; Rust converts one RGBA frame at a time, streams samples into a budgeted unnamed MP4 descriptor, validates the completed file, and publishes that inode without replacement. An isolated functional harness treats the executable as a black box and reopens its MP4 with a separately implemented development-only parser.

**Tech Stack:** Rust 2024 for the worker and functional harness (the shared protocol crate remains Rust 2021), `video-creater-permissive-media-protocol`, serde/serde_json, `mp4` 0.14.0, sha2, libc, OpenH264 2.6.0, libxaac 0.1.13, pinned Rust 1.97.1/Clang 18/musl toolchain, Node 24 build orchestration.

**Spec:** [`docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`](../specs/2026-09-12-linux-compatibility-design.md)

**Protocol prerequisite:** [`docs/superpowers/plans/2026-09-12-linux-permissive-media-contract.md`](./2026-09-12-linux-permissive-media-contract.md) must be implemented and reviewed first.

**Evidence prerequisite:** [`docs/research/2026-09-12-linux-combined-media-proof.md`](../../research/2026-09-12-linux-combined-media-proof.md) is immutable evidence for one fixed fixture. Reuse its pinned inputs and lessons, never its fixture crate as a dependency or its binary hash as worker qualification.

## Global Constraints

- Create only the new worker crate, its isolated functional harness, and its owned build-tool files listed below. Do not modify the app crate, workspace membership, render routing, Settings/readiness, export profiles, Tauri configuration, macOS code, the combined-proof crate, or accepted evidence.
- Runtime behavior has no Node, FFmpeg, GStreamer, `PATH`, package-manager, network, dynamic-codec, or fallback dependency. LGPL/GPL app-linked or app-loaded libraries remain forbidden.
- The only advertised operations are `Capabilities` and `EncodeMp4`. Internal output inspection used to construct `EncodeMp4Result.probe` is not the public `Probe` operation. Do not advertise `Probe` or `ExtractFrames` until separately implemented and tested.
- The worker consumes exact-size raw RGBA8 and optional interleaved S16LE files from the reviewed protocol. Packet 2B converts canonical PNG/WAV/graphics inputs into these spools; this worker does not parse PNG or WAV.
- The initial public rows are exact, reviewable delivery tuples: 640x360 and 1280x720; 24/1 and 30000/1001 fps; 48 kHz stereo when audio is present; and the correlated profiles listed below. Stateful codec APIs remain dimension/frame-count/rational-rate driven so later rows do not require another fixture rewrite.
- Stream one decoded source frame, one I420 frame, and one encoded access unit at a time. Sample-table metadata may grow only to the request's already bounded frame counts. Never collect all raw or encoded samples in memory.
- Use caller budgets as hard ceilings in addition to the worker limits. Check elapsed wall time and cancellation between every frame, audio block, mux write, and finalization phase.
- Write only to an unnamed inode created beneath the supplied output root with `O_TMPFILE`. Sync and validate that owned descriptor, then atomically publish that exact inode without replacement through the held output-directory descriptor using the documented unprivileged `/proc/self/fd/<fd>` plus `linkat(AT_SYMLINK_FOLLOW)` path, and sync the parent directory. `O_TMPFILE` or procfs absence fails closed; there is no named-temporary fallback. Closing an unpublished descriptor is the only cleanup action.
- Audio timing is coded timing. `OwnedAudioProbe.frame_count` is the sum of AAC sample durations in PCM-equivalent frames; microseconds use checked wide arithmetic and floor division. Delay and padding are both `None` in this packet because the pinned AAC-LC API does not report a trustworthy split.
- A passing worker test does not prove gapless audio, full-tail recovery, common MP4, production A/V synchronization, trusted installation, runtime-manifest closure, or `Mp4Primary` readiness.
- Do not rebuild or redownload OpenH264, libxaac, libc++, libunwind, musl, or Rust merely to execute this plan. Consume the accepted pinned prepared inputs/native archives. A changed or missing hash blocks the build instead of triggering a rebuild.
- Before any build, require at least 8 GiB free and set Cargo jobs to 2, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, and `CARGO_PROFILE_TEST_DEBUG=0`. Preserve all accepted proof/evidence roots.

---

## File map

Create these worker files:

- `src-tauri/crates/permissive-media-worker/Cargo.toml` and `Cargo.lock`: standalone `[workspace]`, locked Rust and protocol dependencies, `native-codecs` production feature, static release profile.
- `src-tauri/crates/permissive-media-worker/build.rs`: validate explicit prepared/native inputs and compile only the new bridge objects with the pinned compiler.
- `src-tauri/crates/permissive-media-worker/source-lock.json`: copy the exact upstream revisions, archive hashes, prepared-input hash, native-manifest hash, toolchain identity, and permissive license expressions into a worker-owned lock.
- `src-tauri/crates/permissive-media-worker/notices/*`: worker-owned snapshots for OpenH264, libxaac, LLVM runtime pieces, musl, Rust, and `mp4` 0.14.0.
- `src-tauri/crates/permissive-media-worker/native/codec_bridge.h`, `h264_bridge.cpp`, and `aac_bridge.c`: stateful one-access-unit codec ABI.
- `src-tauri/crates/permissive-media-worker/src/lib.rs`: internal module boundary and `run_one_request` entry used by `main` and tests.
- `src-tauri/crates/permissive-media-worker/src/main.rs`: bounded stdin/NDJSON process adapter and exit-status mapping.
- `src-tauri/crates/permissive-media-worker/src/capabilities.rs`: the exact operation, limit, dimension/rate, audio-mode, and profile rows.
- `src-tauri/crates/permissive-media-worker/src/color.rs`: RGBA8 to BT.709 limited-range I420 conversion for one even-sized frame.
- `src-tauri/crates/permissive-media-worker/src/codec.rs`: safe RAII wrappers over the stateful bridges.
- `src-tauri/crates/permissive-media-worker/src/mp4.rs`: streamed no-B-frame/no-edit-list MP4 writing and owned-subset inspection.
- `src-tauri/crates/permissive-media-worker/src/job_io.rs`: `openat2` input/root checks, exact-size reads, extent-budgeted unnamed output, exact-inode no-replace publish, hashing, and close-only cleanup.
- `src-tauri/crates/permissive-media-worker/src/worker.rs`: operation dispatch, budgets, progress, cancellation, duration/result construction, and typed failures.
- `src-tauri/crates/permissive-media-worker/tests/streaming.rs`: pure state-machine, budget, timing, and injected sink-failure tests.

Create these build and black-box test files:

- `scripts/build-permissive-media-worker.mjs`: structured-argv build/test driver over explicit accepted input roots; no shell interpolation or downloads.
- `scripts/build-permissive-media-worker.test.ts`: mutation tests for input hashes, target/toolchain identity, bridge argv, Cargo flags, and output selection.
- `native/linux-permissive-media-worker-functional/Cargo.toml`, `Cargo.lock`, and `src/lib.rs`: standalone black-box harness depending on the protocol crate, never the worker crate or app crate.
- `native/linux-permissive-media-worker-functional/src/mp4_reopen.rs`: independent bounded ISO BMFF box/sample-table reader; do not use the worker's `mp4.rs` or the `mp4` crate.
- `native/linux-permissive-media-worker-functional/tests/worker.rs`: real worker scenarios, process cancellation, filesystem assertions, and independent reopen checks.

## Binding public behavior

Advertise `MediaProtocolLimits` exactly as:

```rust
pub const WORKER_LIMITS: MediaProtocolLimits = MediaProtocolLimits {
    max_wall_time_ms: 3_600_000,
    max_input_bytes: 68_719_476_736, // 64 GiB
    max_output_bytes: 8_589_934_592, // 8 GiB
    max_video_frames: 108_000,
    max_pixels_per_frame: 921_600,   // 1280x720
    max_audio_frames: 172_800_000,   // one hour at 48 kHz
};
```

Return only `MediaOperationKind::Capabilities` and `MediaOperationKind::EncodeMp4`. Build eight `EncodeCapability` rows: one per tuple below and one copy of each for audio present/absent. Every row has `min_width == max_width`, `min_height == max_height`, `width_multiple = 2`, `height_multiple = 2`, one frame rate, and one full `MediaOutputProfile`. Audio rows use `Some(S16leInterleaved)`, `[48_000]`, `[2]`; video-only rows use `None`, `[]`, `[]`.

| Size | Frame rate | Video bitrate | GOP frames | Audio bitrate |
| --- | --- | ---: | ---: | ---: |
| 640x360 | 24/1 | 800,000 | 48 | 128,000 |
| 640x360 | 30000/1001 | 1,000,000 | 60 | 128,000 |
| 1280x720 | 24/1 | 2,500,000 | 48 | 128,000 |
| 1280x720 | 30000/1001 | 3,000,000 | 60 | 128,000 |

Use these internal interfaces:

```rust
pub(crate) struct EncodeCommand {
    pub request_id: String,
    pub caller_job_root: PathBuf,
    pub output_root: PathBuf,
    pub output_path: PathBuf,
    pub video: RawVideoInput,
    pub audio: Option<RawAudioInput>,
    pub profile: MediaOutputProfile,
    pub budgets: MediaBudgets,
}

pub(crate) trait CancellationCheck: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

pub(crate) fn encode_mp4(
    command: EncodeCommand,
    cancellation: &dyn CancellationCheck,
    emit: &mut dyn FnMut(MediaEvent) -> Result<(), WorkerFailure>,
) -> Result<EncodeMp4Result, WorkerFailure>;

pub(crate) struct VideoEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub frame_rate: RationalFrameRate,
    pub bitrate: u32,
    pub keyframe_interval_frames: u32,
}

pub(crate) struct EncodedVideoAccessUnit {
    pub avcc: Vec<u8>,
    pub is_sync: bool,
}

pub(crate) struct EncodedAudioAccessUnit {
    pub bytes: Vec<u8>,
    pub duration_frames: u32, // exactly 1024 for this AAC-LC profile
}
```

The H.264 bridge creates one encoder from `VideoEncoderConfig`, accepts one I420 frame per call, rejects frame skip, and returns one AVCC access unit plus sync status. Retrieve SPS/PPS from the first access unit before starting the MP4 writer. The AAC bridge creates one encoder from total input byte length, 48 kHz/stereo/128 kbps, exposes its required input-block size, zero-fills only the final partial block, returns one raw AAC access unit per call, and verifies the final count against libxaac's `expected_frame_count`. Its `finish` call verifies counts and destroys state; do not invent an AAC-LC flush command absent from the pinned API.

The video MP4 timescale is the frame-rate numerator and every video sample duration is the denominator. The audio timescale is 48,000 and every AAC sample duration is 1,024. `coded_video_duration_us` and `coded_audio_duration_us` are floor divisions using `u128` intermediates. The owned probe is reconstructed from the completed MP4 tables; its duration is the maximum coded track duration. Video-only results have no audio probe/duration/delay/padding. Audio results have a coded probe/duration and both delay/padding fields set to `None`.

RGBA conversion uses BT.709 limited-range integer coefficients, clamps to legal Y/Cb/Cr ranges, averages each 2x2 RGB block before chroma conversion, and ignores the alpha byte because Packet 2B supplies already composited pixels:

```text
Y  = clamp(16  + round(( 47R + 157G +  16B) / 256), 16, 235)
Cb = clamp(128 + round((-26R -  87G + 112B) / 256), 16, 240)
Cr = clamp(128 + round((112R - 102G -  10B) / 256), 16, 240)
```

The MP4 subset is `isom/iso2/avc1/mp41`, progressive nonfragmented, one H.264 Baseline video track, optional AAC-LC audio track, zero composition offsets, no B-frames, no edit lists, no encryption, and no external references. The output inspector must validate actual `stts`, `stsz`, `stsc`, `stco`/`co64`, `stss`, codec config, sample bounds, track count, and request consistency before publish.

Stable failure codes are `media.worker.requestInvalid`, `media.worker.unsupported`, `media.worker.boundaryInvalid`, `media.worker.inputInvalid`, `media.worker.inputTruncated`, `media.worker.inputTrailingBytes`, `media.worker.outputExists`, `media.worker.outputBudgetExceeded`, `media.worker.timeout`, `media.worker.cancelled`, `media.worker.codec`, `media.worker.mux`, and `media.worker.internal`. Error text is bounded to 1,024 UTF-8 bytes and never includes source media bytes.

### Task 1: Generalize the streaming codec and MP4 core

**Files:** all files under `src-tauri/crates/permissive-media-worker/` except `src/main.rs`, `src/worker.rs`, plus `scripts/build-permissive-media-worker.mjs` and `scripts/build-permissive-media-worker.test.ts`.

**Interfaces:** Consumes the reviewed protocol crate and the exact prepared/native manifests above. Produces `capabilities::capabilities`, `codec::{H264Encoder, AacEncoder}`, `mp4::Mp4Muxer`, and `job_io::{InputFile, AtomicOutput}` for Task 2.

- [ ] **Step 1: Add RED tests for the build boundary and pure core**

  Test the exact source/native hashes, target, pinned compiler/archiver paths, two-job/debug-disabled Cargo environment, static link flags, and refusal to download or rebuild codecs. In Rust, test all eight capability rows, profile correlation/alignment, exact rational timing (including 30000/1001), overflow rejection, BT.709 color bars and 2x2 chroma averaging, highest-file-extent output budgeting across seeks, no-replace publication, optional-audio table shapes, and injected write/finalize failures that leave no final output.

  Run:

  ```bash
  rtk node --test scripts/build-permissive-media-worker.test.ts
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/task1 --test-core
  ```

  Expected: RED because the builder, bridge, and core interfaces do not exist. The driver must stop before Cargo/native compilation if free space is below 8 GiB.

- [ ] **Step 2: Implement the locked build seam and stateful native bridges**

  Copy the accepted source/license identities into the worker lock and notice snapshots. The builder verifies prepared-input hash `db29d85fd2957f1f1ea1f0e77a6f322619d001e4e029db366a14b224f3c9ead9`, native-manifest hash `ce89fb8f9fe9bc5584846db14cf7d294c83983242157eebf5af9ef335f263e`, and the selected archive hashes before invoking the pinned toolchain. Production links `libopenh264.a` (`5e2bf2…`), `libxaacenc.a` (`221cec…`), `libc++abi.a` (`5d55b5…`), and `libunwind.a` (`2dc072…`); it does not link `libxaacdec.a`. Store full lowercase hashes in `source-lock.json`; abbreviated hashes appear here only for readability.

  Replace the proof's all-samples ABI with create/encode-or-process/finish/free handles. Check every dimension/size conversion before narrowing to native integers, compile the bridges with the pinned Clang/Clang++ arguments, and convert every native allocation/codec error into a Rust error without unwinding across FFI.

- [ ] **Step 3: Implement color, timing, budgeted output, and streamed muxing**

  Convert exactly one RGBA frame into reusable I420 buffers. Hold only the first H.264 access unit until SPS/PPS exist, then stream video/AAC samples to `Mp4Writer<AtomicOutput>`. `AtomicOutput` tracks current position and maximum extent rather than cumulative rewritten bytes, rejects seeks/writes beyond `max_output_bytes`, syncs and inspects its unnamed owned descriptor, publishes that exact inode with no replacement, and closes the unpublished descriptor on `Drop` without unlinking any name.

- [ ] **Step 4: Run GREEN core gates and commit**

  Run the two Step 1 commands again. Expected: Node mutations and all pure/static-musl core tests pass; no accepted proof path changes.

  ```bash
  rtk git add src-tauri/crates/permissive-media-worker scripts/build-permissive-media-worker.mjs scripts/build-permissive-media-worker.test.ts
  rtk git commit -m "feat(linux): add streaming permissive media core"
  ```

### Task 2: Implement the bounded protocol worker

**Files:** `src-tauri/crates/permissive-media-worker/src/main.rs`, `src/worker.rs`, and focused additions to tests within the same new crate.

**Interfaces:** Consumes Task 1's exact capabilities, codec/mux, filesystem, and cancellation interfaces. Produces a no-argument process that reads one v1 `MediaRequest` line and emits a valid v1 event transcript.

- [ ] **Step 1: Write protocol-process RED tests**

  Drive `run_one_request` with serialized requests. Require a 1 MiB stdin ceiling, a 5,000 ms monotonic admission ceiling with cancellation-aware polls no more than 50 ms apart, exactly one nonblank request plus EOF, `Started` first, monotonic progress with nonzero total, exactly one terminal event, flushed NDJSON, zero exit only for `Completed`, and nonzero exit for `Failed`. Count intake against `maxWallTimeMs`, bound encode output paths to 768 UTF-8 bytes, and preserve a safely parsed request ID for typed intake cancellation/timeout without inventing an unsafe identity. Cover wrong identity/version, unknown `Probe`/`ExtractFrames`, missing capability tuple, unsupported bitrate/GOP, odd or unadvertised size, wrong rate/audio shape, input/output/wall/frame/pixel/audio budgets, path escape/symlink/nonregular files, truncated and trailing raw bytes, preexisting final output, and every stable failure code.

  Run:

  ```bash
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/task2 --test-worker
  ```

  Expected: RED because request dispatch and the process adapter do not exist.

- [ ] **Step 2: Implement one-request dispatch and filesystem confinement**

  Parse and validate with the protocol crate and `WORKER_LIMITS`, then require an exact advertised capability row. Atomically open the absolute caller job root with Linux `openat2` and `RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS`; resolve inputs/output parents beneath that held handle using `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS`; require regular exact-size inputs and an existing output root. Never create job/output roots or output parent directories.

  Install SIGTERM/SIGINT handling that only flips a cancellation flag. Check that flag through cancellation-aware request polling and around every blocking unit of work. On handled cancellation with a safely admitted request ID emit `Failed(media.worker.cancelled)`, return nonzero, and close the unpublished unnamed output descriptor; the client supervisor remains responsible for process-group escalation.

- [ ] **Step 3: Stream the encode and construct results from actual bytes**

  Read exactly `width * height * 4` bytes per frame and reject early EOF or one trailing byte after the declared final frame. Read audio in the encoder's requested block size, reject early EOF/trailing bytes, zero-fill only the final declared partial block, and verify emitted AU count. Emit progress in source units without one event per byte. After mux finalization, inspect tables, hash the file, construct coded timing/probe fields, publish, then emit `Completed`.

- [ ] **Step 4: Run GREEN worker gates and commit**

  Run the Step 1 command again. Expected: all protocol-process cases pass under the static-musl target, and cancellation/budget/failure cases leave no final output and no named temporary. The explicit committed-but-directory-sync-failed outcome is the exception: the final inode exists for recovery, no `Completed` event is emitted, and no temporary name exists.

  ```bash
  rtk git add src-tauri/crates/permissive-media-worker
  rtk git commit -m "feat(linux): add bounded permissive media worker"
  ```

### Task 3: Prove variable real outputs through an independent harness

**Files:** all files under `native/linux-permissive-media-worker-functional/`, plus focused build-driver additions required to launch the release worker and harness.

**Interfaces:** Consumes only the protocol crate and an explicit worker executable path selected by the owned builder. Produces black-box functional evidence and a fresh worker build receipt; it does not produce a runtime manifest or readiness decision.

- [ ] **Step 1: Write black-box RED scenarios and the independent MP4 reader**

  The harness must not import the worker crate or `mp4` 0.14.0. Its bounded parser walks box sizes with checked arithmetic, rejects overlap/truncation, resolves sample tables to `mdat` extents, and reports codec entries, dimensions, timescales, durations, sample counts, sync samples, and zero composition offsets.

  Add these real scenarios:

  1. 640x360, 24/1, 37 RGBA frames, no audio: completed video-only MP4, exact 1,541,666 µs coded video duration, no audio fields, and distinct bounded early/middle/late access-unit payloads for the changing source bars. Exact pre-encode I420 color values remain the responsibility of Task 1's color tests; compressed-payload inequality is not a decoder-conformance claim.
  2. 640x360, 30000/1001, 61 frames plus 97,137 stereo audio frames: more-than-one-second/non-AAC-block-aligned input, 61 video samples, 95 AAC access units, 97,280 coded audio frames, 2,026,666 µs coded audio duration, and both delay/padding fields `None`.
  3. 1280x720, 24/1, three frames: short standard-delivery-size smoke with the exact 2.5 Mbps/GOP-48 profile.
  4. One-byte-short and one-byte-long video/audio spools, missing/nonregular input, unsupported size/rate/profile, output budget exhausted during mux finalization, preexisting target, and a file truncated while being read: typed failure, nonzero exit, no published/partial artifact. Separately truncate copies of each successful MP4 at box headers, sample tables, and media payloads and require the independent reader to reject them.
  5. A long encode cancelled immediately after its first `Progress`: bounded exit, nonzero status, and no published/partial artifact.

  Run:

  ```bash
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/functional --release --functional
  ```

  Expected: RED because the harness/parser and functional driver do not exist.

- [ ] **Step 2: Make the functional matrix GREEN without weakening assertions**

  Generate deterministic raw files incrementally so the harness itself does not retain a full 720p sequence. Spawn with structured argv and a cleared environment, validate the entire NDJSON transcript through protocol types, reopen every success with the independent parser, and compare its facts with the result. Cancellation sends SIGTERM to the worker process group and enforces the same bounded cleanup observation as the production supervisor.

- [ ] **Step 3: Run the focused static and scope gates**

  Run:

  ```bash
  rtk node --test scripts/build-permissive-media-worker.test.ts
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/final --release --test-core --test-worker --functional --emit-receipt
  rtk cargo fmt --manifest-path src-tauri/crates/permissive-media-worker/Cargo.toml --all -- --check
  rtk cargo fmt --manifest-path native/linux-permissive-media-worker-functional/Cargo.toml --all -- --check
  rtk git diff --check
  ```

  Expected: all focused tests pass; the selected release file is x86-64 static PIE with no ELF interpreter, `DT_NEEDED`, or undefined codec symbols; the normal Cargo graph contains no Tauri, GTK, WebKitGTK, GStreamer, FFmpeg, or LGPL/GPL package. The receipt records the new source tree, exact input/archive hashes, compiler/link argv, selected executable hash, `file`/`readelf` facts, and test results. It is development evidence, not the future signed runtime manifest/source-contributor/license/SBOM closure.

- [ ] **Step 4: Commit the isolated harness**

  ```bash
  rtk git add native/linux-permissive-media-worker-functional scripts/build-permissive-media-worker.mjs scripts/build-permissive-media-worker.test.ts
  rtk git commit -m "test(linux): qualify permissive media worker slice"
  ```

## Audio and release boundary

The pinned libxaac documentation says delay adjustment applies only to AOT 42, while this contract uses AAC-LC (AOT 2). Its AAC-LC output configuration provides output bytes, bytes consumed, input size, and expected frame count, but no reviewed encoder-delay/padding result and no documented encoder input-over flush. The accepted one-second proof produced 48,128 decoded frames from 48,000 input frames and found a 1,600-frame content lag only in a central window. Those facts do not establish that the source tail survives.

Packet 2A therefore reports exact coded duration and `None`/`None` for delay/padding. Before any Linux route advertises `Mp4Primary`, a separate reviewed gate must encode start/middle/end impulses and long-run clocks, independently decode the entire audio/video output, prove final-source-tail recovery and drift bounds, and establish trustworthy priming/padding. If the chosen repair requires leading/trailing samples or ISO BMFF edit metadata, add explicit owned edit-list writing, parsing, and hostile-table tests in a later plan; do not silently trim, shift timestamps, discard tail samples, or convert measured correlation into metadata.

After this packet, proceed directly to Packet 2B's canonical PNG/WAV/graphics compositor feeding the exact raw spool contract. Runtime-manifest installation, contributor/license/SBOM closure, `Probe`/`ExtractFrames`, common MP4 import, WebM, transcription, canonical routing, readiness, and packaging remain separate release gates.

## Completion boundary

Passing this plan proves a generalized static-musl encoder worker for eight exact capability rows, bounded streaming behavior, atomic output, truthful coded timing, and independently reopened owned-subset MP4 files. It does not alter macOS, make the worker resolvable by production, route any canonical render, claim full-tail/gapless/sync behavior, or complete Linux compatibility.
