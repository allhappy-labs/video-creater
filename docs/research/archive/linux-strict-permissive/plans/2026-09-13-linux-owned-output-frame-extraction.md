# Linux Owned-Output Frame Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bounded v1 `ExtractFrames` operation to the static-musl permissive media worker so Packet 2C can review pixels decoded from the worker's narrowly admitted progressive H.264 output shape.

**Architecture:** Reuse the worker's accepted fixed-subset MP4 validation and already linked OpenH264 archive. A descriptor-confined reader derives only the video facts needed for extraction, then a pure-Rust bounded H.264 admission pass validates the actual SPS, PPS, and every access-unit slice prefix before any native decoder allocation or feed. Only the worker encoder's exact no-B-frame/no-extension shape reaches a stateful bridge. The worker selects samples by presentation interval, converts one returned I420 frame to raw RGBA8, and publishes a small bounded set of outputs through the existing unnamed-inode/no-replace machinery. The public `Probe` operation remains unsupported; the accepted `EncodeMp4Result.probe` stays the metadata authority for the immediately encoded artifact.

**Tech Stack:** Rust 2024, the unchanged v1 `video-creater-permissive-media-protocol`, `mp4` 0.14.0, sha2, libc, OpenH264 revision `652bdb7719f30b52b08e506645a7322ff1b2cc6f`, pinned Rust 1.97.1/Clang 18/musl inputs, Node 24 build orchestration.

**Spec:** [`docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`](../specs/2026-09-12-linux-compatibility-design.md)

**Roadmap:** [`docs/superpowers/plans/2026-09-12-linux-full-compatibility.md`](./2026-09-12-linux-full-compatibility.md), prerequisite W1 for Packet 2C.

**Accepted baseline:** Worker source through `83d91694`; protocol/client through `0ad24aa` and `6ffb81fd`; Packet 2B through `6569f3d2`. The retained worker receipt at `output/linux-permissive-media-worker/final-packet-review-fix/build-receipt.json` has SHA-256 `5ad91acf8275ee6e7e335f17076880c7b3e74b2bd2830faef04f7fd6d6ad3058` and records worker SHA-256 `847ea8c2197f17c8e1d06d4678ed68d3aac2b87a4852bd26a8e9cc5ff4268b71`. Treat that root as immutable pre-change evidence.

## Preflight and ownership

| Gate | State before implementation | Rule |
| --- | --- | --- |
| Root ruling R1-A | Accepted | Implement `ExtractFrames` only. `Probe` remains typed unsupported. |
| Protocol v1 | Accepted and frozen | Keep positive `presentationTimeUs`, ordered results, and raw RGBA files. Do not add schema fields or version 2. |
| Native decoder input | Accepted and already linked | Reuse the exact `libopenh264.a` hash `5e2bf20844cdf787c67f7e629f9d8866a6bfae78085eaee93fd0ae2d719291a6`, BSD-2-Clause notice, and prepared source tree read-only. |
| Combined proof | Read-only lesson source | Reuse the `DecodeFrameNoDelay`, AVCC-to-Annex-B, stride-copy, flush, and RAII lessons. Do not depend on its fixture crate, copy its fixed 30-frame ABI, or relabel its evidence. |
| Packet 2B | Accepted at `6569f3d2` | No spool implementation changes belong to W1. |
| Other work | Concurrent | Preserve all Desktop test changes and every file outside the W1 map below. |
| Plan authorization | Root review required | Do not implement W1 until root accepts this child plan. |

W1 owns only these source areas when implementation begins:

- `src-tauri/crates/permissive-media-worker/`
- focused additions to `native/linux-permissive-media-worker-functional/`
- focused additions to `scripts/build-permissive-media-worker.mjs` and `scripts/build-permissive-media-worker.test.ts`

It does not own the protocol/client, app render route, Packet 2B spools, Packet 4A, Font Gate A, Settings, roadmap/spec, accepted reports, or Desktop lane.

## Binding behavior and limits

- `capabilities()` adds only `MediaOperationKind::ExtractFrames`. The eight existing `EncodeCapability` rows, `WORKER_LIMITS`, profiles, and `EncodeMp4` behavior remain byte-for-byte equivalent. `Probe` is not advertised and still returns `media.worker.unsupported`.
- Extraction accepts only the structurally validated progressive `isom/iso2/avc1/mp41` subset already emitted by this worker: one H.264 Baseline video track, optional AAC-LC 48 kHz stereo, no fragments, edit lists, external references, encryption sample entries, B-frame composition offsets, variable frame timing, or unsupported boxes/tables. Video size and rational rate must be one of the four existing encode tuples.
- MP4 labels, `avcC`'s Baseline byte, and zero composition offsets are necessary container checks, but they do not admit compressed video by themselves. Before creating or feeding OpenH264, parse and validate the actual `avcC` SPS/PPS RBSP and every video sample's actual NAL and slice-header prefix against the source-grounded profile below. Reject a file if the labels and RBSP disagree.
- Structural admission is not cryptographic provenance. A foreign encoder could create the same shape. Name the scope `owned-worker-output-v1` in tests and receipts, never `common MP4`, and do not claim authorship from atoms, dimensions, or hashes alone.
- Requested `width` and `height` must equal the coded video dimensions. W1 has no scaler and admits no arbitrary size.
- The v1 timestamp is an ordered request selector and is echoed unchanged in `ExtractedFrame.presentation_time_us`; it is not reported as the selected sample's actual DTS or PTS. Packet 2C will translate canonical time zero to protocol time `1` microsecond and retain canonical zero separately in its report. W1 proves that `1` microsecond selects sample index zero.
- For a sample with start tick `s`, duration `d`, and track timescale `t`, timestamp `u` selects that sample exactly when `s * 1_000_000 <= u * t < (s + d) * 1_000_000`, using checked `u128` arithmetic. Reject a request at or after track end. At an integer-microsecond boundary use the first microsecond whose scaled value reaches the new sample; tests calculate that boundary with ceiling division.
- Define `MAX_EXTRACTION_OUTPUTS: usize = 16`. Reject a larger output list with `media.worker.unsupported`; this is the explicit v1 limitation that keeps the prepublication descriptor set bounded. Source decoding may still walk up to the caller/worker `max_video_frames` ceiling.
- Open the source once beneath the held caller-job-root descriptor with `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)`. Require a nonempty regular file no larger than `max_input_bytes`, retain that descriptor through parse/decode, and never reopen the source path. The accepted client continues to snapshot inode/length/SHA-256 before launch and revalidate them after exit.
- Enforce all caller budgets as hard ceilings: source bytes, parsed video/audio sample counts, coded pixels, requested output count, exact aggregate `width * height * 4 * frames.len()`, and the monotonic deadline. Keep `MAX_OWNED_SAMPLE_BYTES = 16 MiB`; set `MAX_PARAMETER_SET_BYTES = 256`, `MAX_NALS_PER_VIDEO_SAMPLE = 1`, and bound Annex-B growth before allocation. The parameter-set cap is a local policy ceiling over the worker's small SPS/PPS, not a general H.264 limit.
- Run a complete first pass over all bounded video samples before native decoder creation. Validate each four-byte AVCC length and slice prefix and retain only one SHA-256 per admitted sample plus bounded timing/index metadata; do not retain sample payloads. Size the digest vector with checked arithmetic under `max_video_frames`. During the decode pass, hash and revalidate each read AU before feed; reject an in-place mutation if its digest or syntax differs. Hold at most one compressed AU, one Annex-B AU, one I420 frame, and one RGBA frame while decoding.
- Allocate all requested `AtomicOutput` descriptors, validate all parent paths/target absence, decode all selected frames, write exact RGBA bytes, hash them, and finish the decoder before publishing any name. Any request/parse/decode/write/hash/cancel/timeout failure before publication leaves no final output.
- v1 names multiple independent output paths, so Linux cannot commit the whole set with one filesystem operation. Publish each already complete unnamed inode without replacement only after all decoding succeeds. Emit `Completed` only after every output is durable. If a later link or directory sync fails, emit bounded `media.worker.internal` recovery detail naming the already committed relative paths, preserve those exact inodes, and rely on the later canonical job owner to quarantine the render directory. Never race an identity check followed by `unlinkat`, never delete a replacement, and never claim all-or-none publication.
- Result order exactly matches request order, including repeated timestamps to different unique paths. Every result reports the requested relative path/time, coded width/height, exact `width * height * 4` length, and SHA-256 of the actual decoded raw RGBA8 file. Alpha is `255` for every pixel.
- Check cancellation and deadline before/after source open, table parsing phases, every sample read/decode, every RGBA write/hash, and every publish. Preserve `Started`, bounded monotonic `Progress`, one terminal event, flush behavior, and exit-status parity.
- No new crate, native archive, source revision, notice, runtime tool, model, download, or `PATH` lookup is permitted. If implementation discovers a required dependency, stop and return to root for a new source/license/feature-gate ruling.
- If the first actual decode fixtures across the eight accepted encode rows contradict any hard-coded source-derived SPS/PPS/AU field, stop and report the exact bytes and source path to root for a narrow admission-plan correction. Never change the accepted encoder configuration, capability rows, or receipt claims merely to make extraction accept the fixture.
- This is private development evidence. It does not establish public release authority, patent clearance, signed installation, common decode, arbitrary resize, full Linux readiness, or Mac verification.

## Interfaces to implement

Extend `job_io.rs` without weakening existing exact raw-input opens:

```rust
impl InputFile {
    pub fn open_beneath_at_most(
        root: &File,
        relative: &Path,
        maximum: u64,
    ) -> Result<Self, JobIoError>;
}

```

Keep `open_beneath(root, relative, expected)` for raw encode inputs. Implement `std::io::Seek` for `InputFile`, rejecting seeks outside its recorded length. The new open method rejects zero-length, oversized, symlink, escaped, and nonregular sources and records the actual descriptor length.

Add a narrow reader in `mp4.rs`:

```rust
pub struct OwnedSubsetVideo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: RationalFrameRate,
    pub frame_count: u64,
    pub duration_ticks: u64,
}

pub struct OwnedSubsetReader {
    reader: Mp4Reader<BufReader<InputFile>>,
    video_track_id: u32,
    video: OwnedSubsetVideo,
    sequence_parameter_set: Vec<u8>,
    picture_parameter_set: Vec<u8>,
}

impl OwnedSubsetReader {
    pub fn open(input: InputFile, budgets: &MediaBudgets) -> Result<Self, OwnedReadFailure>;
    pub fn video(&self) -> &OwnedSubsetVideo;
    pub fn select_frame(&self, presentation_time_us: u64) -> Result<u64, OwnedReadFailure>;
    pub fn read_video_access_unit(&mut self, frame_index: u64)
        -> Result<EncodedVideoAccessUnit, OwnedReadFailure>;
}
```

Refactor `inspect_owned` to share the accepted raw-table validation floor with this reader, while retaining its request-derived `Mp4Expectations` comparisons for encode. The extraction reader derives only internal facts needed to reject out-of-scope media and select samples; it does not produce `MediaResult::Probe`.

Add `h264_admission.rs`. Its bounded bit reader owns EBSP-to-RBSP removal, rejects malformed emulation-prevention bytes, never reads past its NAL, and rejects an unsigned Exp-Golomb code before shifting when it has more than 31 leading zero bits. Each parsed field also has the tighter fixed-profile bound below. The only way to obtain decoder bytes is through private fields on an admitted value:

```rust
pub struct AdmittedDecoderConfig {
    // Private: validated SPS/PPS and checked picture/reference budgets.
}

pub struct AdmittedH264 {
    decoder: AdmittedDecoderConfig,
    sample_sha256: Vec<[u8; 32]>,
}

impl AdmittedH264 {
    pub fn scan(
        reader: &mut OwnedSubsetReader,
        budgets: &MediaBudgets,
        check_guard: &mut dyn FnMut() -> Result<(), OwnedReadFailure>,
    ) -> Result<Self, OwnedReadFailure>;

    pub fn decoder_config(&self) -> &AdmittedDecoderConfig;
    pub fn validate_access_unit(
        &self,
        sample_index: u64,
        access_unit: &[u8],
    ) -> Result<(), OwnedReadFailure>;
}
```

`scan` first admits `avcC`, then visits every sample and returns only after the entire compressed stream is admitted. It checks cancellation/deadline around every sample. `validate_access_unit` repeats the bounded syntax check and compares the stored digest immediately before a native feed, closing mutation of the held descriptor between passes.

The admission profile comes from the accepted worker's `native/h264_bridge.cpp` configuration and the exact retained OpenH264 source at revision `652bdb7719f30b52b08e506645a7322ff1b2cc6f` (`param_svc.h`, `au_set.cpp`, `svc_encode_slice.cpp`, and `ref_list_mgr_svc.cpp`). It deliberately does not attempt to recognize all valid H.264:

| Syntax | Admitted worker-output-v1 value |
| --- | --- |
| `avcC` | Version 1; four-byte NAL lengths; exactly one SPS and one PPS, each `1..=256` bytes; header profile/compatibility/level bytes exactly equal the parsed SPS. |
| SPS profile and identifiers | Baseline `profile_idc = 66`; constraint sets 0 and 1 set and all other constraint/reserved bits clear; `seq_parameter_set_id = 0`; level 3.0 for 640x360 rows and level 3.1 for 1280x720 rows. |
| SPS prediction/reference shape | `log2_max_frame_num_minus4 = 11`; `pic_order_cnt_type = 2`; `max_num_ref_frames = 1`; gaps forbidden; `frame_mbs_only_flag = 1`; no separate colour plane, interlace, scaling matrices, or profile extensions. |
| SPS dimensions | Macroblock grid is exactly 40x23 with the exact bottom crop producing 640x360, or 80x45 with no crop producing 1280x720. Checked cropped output dimensions must equal the MP4 sample entry and request; the decoder has no resize path. |
| SPS VUI | The emitted restricted VUI only: no aspect/overscan/video-signal/chroma-location/timing/HRD/pic-structure sections; bitstream restriction present; motion vectors over picture boundaries allowed; byte/bit denominators zero; horizontal/vertical motion-vector limits 16; `max_num_reorder_frames = 0`; `max_dec_frame_buffering = 1`. Reject absent, extra, or different VUI syntax for W1. |
| PPS | `pic_parameter_set_id = 0` references SPS 0; CAVLC; one slice group; default L0/L1 references each one; weighted prediction off; initial QP/QS/chroma deltas zero; deblocking-control present; constrained-intra and redundant-picture flags clear; valid RBSP trailing bits. |
| Access unit | Exactly one nonempty AVCC NAL, hence one VCL slice, per MP4 video sample. Admit only type 5 IDR or type 1 non-IDR with nonzero `nal_ref_idc`. Reject SPS/PPS in-band, AUD, SEI, partitions, prefix/subset SPS, SVC/MVC/other extension NALs, and every other NAL type. |
| Slice prefix | `first_mb_in_slice = 0`; exact `slice_type` 0 (P) or 2 (I), rather than modulo-5 aliases; PPS id 0; 15-bit frame number. Type 5 must be I with bounded IDR id, `no_output_of_prior_pics_flag = 0`, and `long_term_reference_flag = 0`. A type 1 P slice may use only one L0 reference and the encoder's single bounded reorder command (short-term idc 0, `abs_diff_pic_num_minus1 = 0`, then idc 3); a referenced non-IDR picture must have `adaptive_ref_pic_marking_mode_flag = 0`. Reject B/SP/SI slices and any long-term/MMCO or second-reference syntax. |

For both fixed sizes, calculate macroblock-aligned I420 bytes with checked arithmetic. Require `max_num_ref_frames = max_dec_frame_buffering = 1`, `MAX_ADMITTED_DPB_BYTES = 1_382_400`, and a caller-side working-decode ceiling of three largest aligned I420 pictures (`4_147_200` bytes: one admitted DPB picture plus current/output allowance) before native allocation. These are admission ceilings for syntax-driven picture/reference storage, not a claim about every byte OpenH264 may allocate internally.

Extend the existing OpenH264 ABI in `native/codec_bridge.h`, `native/h264_bridge.cpp`, and safe RAII wrappers in `codec.rs`:

```rust
pub struct H264Decoder {
    handle: NonNull<c_void>,
}

impl H264Decoder {
    pub fn create(config: &AdmittedDecoderConfig) -> Result<Self, CoreError>;

    pub fn decode(&mut self, avcc: &[u8]) -> Result<I420Frame, CoreError>;
    pub fn finish(self) -> Result<(), CoreError>;
}
```

No public/raw constructor accepts unparsed SPS/PPS. The bridge converts only a revalidated admitted four-byte-length AVCC VCL NAL to Annex-B, prepends the admitted SPS/PPS pair once, runs single-threaded `DecodeFrameNoDelay`, validates returned dimensions/strides, and returns exactly one owned I420 frame for every admitted no-B-frame sample. `finish` rejects buffered or extra frames. All handles and native allocations use RAII on success, failure, cancellation, and panic boundaries. Add a narrow internal decoder-factory seam so tests can prove invalid SPS/PPS or any invalid later AU produces zero decoder-create and zero decoder-feed calls; production binds that seam directly to the RAII owner.

Add inverse color conversion beside the accepted encoder conversion:

```rust
impl I420Frame {
    pub fn to_rgba8_bt709_limited(&self, output: &mut [u8]) -> Result<(), CoreError>;
}
```

Use checked plane sizes and the BT.709 limited-range integer inverse (`298/459`, `298/-55/-136`, `298/541` coefficients), clamp RGB to `0..=255`, and write alpha `255`. Tests compare decoded lossy pixels with declared tolerances; they do not require the decoded hash to equal the pre-encode RGBA hash.

Add the command boundary in `worker.rs`:

```rust
pub(crate) struct ExtractCommand {
    pub request_id: String,
    pub caller_job_root: PathBuf,
    pub source_path: PathBuf,
    pub output_root: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<FrameExtraction>,
    pub budgets: MediaBudgets,
}

fn extract_frames(
    command: ExtractCommand,
    cancellation: &dyn CancellationCheck,
    deadline: Instant,
    hooks: &dyn ExecutionHooks,
    emit: &mut dyn FnMut(MediaEvent) -> Result<(), WorkerFailure>,
) -> Result<ExtractFramesResult, WorkerFailure>;
```

The implementation sorts/deduplicates selected source indices only for decode work, then restores the original request order when filling outputs/results. It must not confuse a request timestamp with sample timing.

---

### Task 1: Add the fixed-subset reader and streaming decoder core

**Files:**

- Modify `src-tauri/crates/permissive-media-worker/src/lib.rs`
- Modify `src-tauri/crates/permissive-media-worker/src/job_io.rs`
- Modify `src-tauri/crates/permissive-media-worker/src/mp4.rs`
- Create `src-tauri/crates/permissive-media-worker/src/h264_admission.rs`
- Modify `src-tauri/crates/permissive-media-worker/src/codec.rs`
- Modify `src-tauri/crates/permissive-media-worker/src/color.rs`
- Modify `src-tauri/crates/permissive-media-worker/native/codec_bridge.h`
- Modify `src-tauri/crates/permissive-media-worker/native/h264_bridge.cpp`
- Modify `src-tauri/crates/permissive-media-worker/tests/streaming.rs`
- Modify `scripts/build-permissive-media-worker.mjs`
- Modify `scripts/build-permissive-media-worker.test.ts`

**Produces:** The bounded input descriptor, structurally strict owned-subset reader, full-stream pre-native H.264 admission, exact timestamp selector, stateful one-AU OpenH264 decoder, I420-to-RGBA conversion, and a recorded host-core test step. It does not change advertised operations yet.

- [ ] **Step 1: Add RED tests for source admission, selection, decode, and build identity**

  In `streaming.rs`, build small deterministic MP4s with the existing muxer and add tests named for these behaviors:

  - `bounded_source_open_holds_the_original_inode_and_refuses_escape_symlink_empty_and_oversize`
  - `owned_subset_reader_accepts_only_fixed_progressive_rows_and_bounded_samples`
  - `presentation_intervals_select_first_middle_final_and_cut_adjacent_frames`
  - `one_microsecond_selects_frame_zero_without_becoming_its_dts`
  - `h264_admission_rejects_large_sps_dimensions_and_reference_count_before_decoder_create`
  - `h264_admission_rejects_avcc_header_sps_mismatch_before_decoder_create`
  - `h264_admission_rejects_midstream_parameter_sets_before_decoder_create`
  - `h264_admission_rejects_b_slices_and_extension_nals_before_decoder_create`
  - `h264_admission_rechecks_sample_identity_before_every_decoder_feed`
  - `native_decoder_streams_one_frame_per_access_unit_and_releases_on_failure`
  - `decoded_i420_converts_to_exact_length_rgba_with_opaque_alpha`

  Cover 24/1 and 30000/1001 with checked boundary ceiling arithmetic. Mutate generated files to reject truncated box/sample payloads, fragments, edit lists, nonzero composition offsets, variable `stts`, missing/multiple/oversized SPS/PPS, non-four-byte NAL lengths, unsupported dimensions/rates, oversize samples, and invalid AAC companion tracks. Add bit-exact adversarial mutations for: an `avcC` Baseline/level tuple that disagrees with the SPS; an SPS whose macroblock dimensions or crop exceed the fixed row; an SPS with excessive `max_num_ref_frames`/VUI reorder or decoder-buffer counts; truncated/overflowing Exp-Golomb and malformed emulation-prevention bytes; an in-band changed SPS/PPS after a valid first sample; B/SP/SI slice types; a second VCL NAL; and prefix/subset-SPS/SVC/MVC/other extension NALs. The spy decoder factory must report zero create and zero feed calls for every whole-file admission failure. A separate held-descriptor mutation test changes a previously admitted sample and proves the digest/syntax recheck rejects it before that sample reaches `decode`.

  Extend the Node mutation tests with a `--test-host-core` plan step. It must use the accepted pinned cargo/rustc and offline lock, omit `native-codecs` on the GNU host, use an output-root-owned target directory, and record the host result in the final receipt. Musl core tests keep `--features native-codecs` and exercise the real decoder.

  Run:

  ```bash
  rtk node --test scripts/build-permissive-media-worker.test.ts
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/owned-extract-task1 --test-host-core --test-core
  ```

  Expected RED: the new reader/decoder APIs and host-core build-plan step do not exist. Input-hash validation must fail before compilation if any accepted archive, tool, or manifest identity changes; the command must never rebuild or download an upstream input.

- [ ] **Step 2: Implement descriptor-bounded parsing and frame selection**

  Add `open_beneath_at_most` and bounded `Seek` while leaving exact encode input behavior unchanged. Split current `inspect_owned` internals into shared strict table validation plus encode-expectation checks. Construct `OwnedSubsetReader` around the same held descriptor and preserve `catch_unwind` around `mp4` header/sample reads.

  Validate actual source length, brands, track count, sample entries, one bounded SPS/PPS pair, fixed timing, sync-table order, sample extents/count/size, optional AAC shape, and all request/worker ceilings before reading a compressed sample. Implement the checked presentation-interval predicate verbatim from this plan.

  Implement the bounded Rust RBSP/bit/Exp-Golomb reader and the exact source-grounded table above. Cross-check `avcC`, parsed SPS/PPS, sample entry, and fixed row; calculate the DPB/working-picture budgets; then preflight every AU and retain checked sample digests. Do not call a native decoder merely to discover dimensions, profile, reference count, POC mode, slice type, parameter-set changes, or extension NALs.

- [ ] **Step 3: Implement one-access-unit decode and inverse color conversion**

  Adapt only the reviewed combined-proof decoder-lifetime and copy lessons into the existing worker bridge. The retained combined proof may contain repeated in-band parameter sets and therefore is not the admitted stream definition. Replace its all-samples/fixed-fixture shape with a stateful create/decode/finish/free ABI, set OpenH264 decoder threads to zero, accept only `AdmittedDecoderConfig`, convert one revalidated bounded AVCC VCL AU to Annex-B, validate the returned I420 dimensions/strides, and copy exactly one frame before the next decoder call. Reject delayed/extra output because the parsed slice syntax, POC/reference bounds, and zero composition offsets jointly exclude B frames.

  Implement the safe Rust owner and inverse BT.709 conversion. Keep a disabled-feature `H264Decoder` stub so host pure tests compile without native archives. Do not add a Cargo dependency, archive, notice, or source-lock revision; the existing `native-codecs` feature remains the only production codec gate.

- [ ] **Step 4: Make focused host and musl core gates GREEN and commit**

  Re-run the Step 1 commands. Expected GREEN: Node mutation tests, host pure tests, and static-musl core/native decoder tests pass from existing locked inputs.

  Format only owned Rust files with the exact Rust 1.97.1 formatter; do not invoke root-workspace `cargo fmt --all`:

  ```bash
  rtk /home/olhapi/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin/rustfmt --edition 2024 --check src-tauri/crates/permissive-media-worker/src/lib.rs src-tauri/crates/permissive-media-worker/src/job_io.rs src-tauri/crates/permissive-media-worker/src/mp4.rs src-tauri/crates/permissive-media-worker/src/h264_admission.rs src-tauri/crates/permissive-media-worker/src/codec.rs src-tauri/crates/permissive-media-worker/src/color.rs src-tauri/crates/permissive-media-worker/tests/streaming.rs
  rtk git diff --check -- src-tauri/crates/permissive-media-worker scripts/build-permissive-media-worker.mjs scripts/build-permissive-media-worker.test.ts
  ```

  Commit only Task 1 files:

  ```bash
  rtk git add src-tauri/crates/permissive-media-worker/src/lib.rs src-tauri/crates/permissive-media-worker/src/job_io.rs src-tauri/crates/permissive-media-worker/src/mp4.rs src-tauri/crates/permissive-media-worker/src/h264_admission.rs src-tauri/crates/permissive-media-worker/src/codec.rs src-tauri/crates/permissive-media-worker/src/color.rs src-tauri/crates/permissive-media-worker/native/codec_bridge.h src-tauri/crates/permissive-media-worker/native/h264_bridge.cpp src-tauri/crates/permissive-media-worker/tests/streaming.rs scripts/build-permissive-media-worker.mjs scripts/build-permissive-media-worker.test.ts
  rtk git commit -m "feat(linux): add bounded owned MP4 decoder core"
  ```

### Task 2: Expose v1 `ExtractFrames` with honest publication semantics

**Files:**

- Modify `src-tauri/crates/permissive-media-worker/src/capabilities.rs`
- Modify `src-tauri/crates/permissive-media-worker/src/worker.rs`
- Modify `src-tauri/crates/permissive-media-worker/tests/worker_process.rs`
- Focused follow-up changes to Task 1 worker files only when a test exposes a real interface defect

**Produces:** `Capabilities + EncodeMp4 + ExtractFrames`, with ordered exact RGBA results. `Probe` remains unsupported. All decode/output work stays inside the current one-request process and event contract.

- [ ] **Step 1: Write protocol-process RED tests**

  Replace the current combined unimplemented-operation assertion with separate facts: `Probe` remains typed unsupported; `ExtractFrames` is advertised and dispatched. Generate source MP4 bytes with worker test helpers rather than embedding a downloaded fixture.

  Cover:

  - request time `1` selects decoded frame zero while the result echoes `1`;
  - first, middle, final, and both sides of a calculated cut boundary;
  - out-of-request-order times and duplicate times preserve requested result/path order;
  - exact coded dimensions only, exact `width * height * 4` lengths, opaque alpha, actual file hashes, and pairwise-distinct changing frames;
  - zero time rejected by the existing protocol before dispatch and track-end/oversize-output-list requests rejected without publication;
  - malformed, truncated, structurally foreign, wrong-size, wrong-rate, symlink, escape, nonregular, vanished, and oversized sources;
  - preexisting output, output-parent symlink/escape, aggregate output budget, per-sample/input/frame ceilings, cancellation, deadline, decoder error, and injected publication failure;
  - no names on any prepublication failure; explicit preserved committed paths on an injected failure after the first publish; foreign replacements are never removed;
  - `Started`, bounded monotonic `Progress`, one terminal event, flushed NDJSON, and exit parity for success/failure.

  Run:

  ```bash
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/owned-extract-task2 --test-worker
  ```

  Expected RED: capability and dispatch still reject `ExtractFrames`.

- [ ] **Step 2: Implement capability and command admission**

  Append `ExtractFrames` to `MediaCapabilities.operations`; do not alter the eight encode rows or numeric limits. In dispatch, leave `Probe` in the typed unsupported arm and map `ExtractFrames` into `ExtractCommand`.

  Before codec creation, require at most 16 outputs, checked exact aggregate RGBA bytes within `max_output_bytes`, exact coded/request dimensions, source facts within budgets, safe source/output paths, existing real output parents, and absence of every final target. Open all final parent directories and create all unnamed outputs, complete `AdmittedH264::scan` over every AU, and calculate the bounded decoder configuration before any native decoder allocation or compressed-byte feed.

- [ ] **Step 3: Decode selected samples and construct exact results**

  Calculate each requested source index with the binding interval rule, group requests by index for one forward decode pass, and decode from sample zero through the highest selected index. Check the guard around every sample/codec/output unit. Immediately before each feed, repeat AU syntax validation and compare its first-pass digest. Copy the decoded RGBA bytes to every request mapped to that selected index, hash the held descriptor, and construct claims in original order.

  Finish and validate the decoder before publication. Publish in request order, preserving the explicit partial-commit recovery outcome if a later publish fails. Validate `MediaResult::ExtractFrames(result.clone())` before the first link. Emit `Completed` only after all links and directory syncs succeed.

- [ ] **Step 4: Run process GREEN gates and commit**

  Run the Step 1 command plus the Task 1 host/core commands. Expected GREEN: existing encode/process cases and all new extraction cases pass; `Probe` still fails with `media.worker.unsupported`.

  ```bash
  rtk /home/olhapi/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin/rustfmt --edition 2024 --check src-tauri/crates/permissive-media-worker/src/capabilities.rs src-tauri/crates/permissive-media-worker/src/worker.rs src-tauri/crates/permissive-media-worker/tests/worker_process.rs
  rtk git diff --check -- src-tauri/crates/permissive-media-worker
  rtk git add src-tauri/crates/permissive-media-worker/src/capabilities.rs src-tauri/crates/permissive-media-worker/src/worker.rs src-tauri/crates/permissive-media-worker/tests/worker_process.rs
  rtk git commit -m "feat(linux): expose owned output frame extraction"
  ```

### Task 3: Prove actual decoded RGBA through the black-box worker

**Files:**

- Modify `native/linux-permissive-media-worker-functional/tests/worker.rs`
- Modify `native/linux-permissive-media-worker-functional/src/mp4_reopen.rs` only if the independent parser needs a bounded helper to calculate sample boundaries
- Modify `scripts/build-permissive-media-worker.mjs`
- Modify `scripts/build-permissive-media-worker.test.ts`

**Produces:** Black-box static-musl evidence that extraction reads the actual just-encoded MP4 and writes decoded raw RGBA. It does not use worker internals or a pre-encode PNG as rendered-frame proof.

- [ ] **Step 1: Add black-box RED scenarios**

  Extend the harness request/result helpers to handle both `EncodeMp4Result` and `ExtractFramesResult`. For each success, first invoke the release worker's real `EncodeMp4`, retain that MP4 as `sourcePath`, then invoke its real `ExtractFrames` in a second process. Never construct an extraction success from raw source bytes alone.

  Add these scenarios:

  1. 640x360 at 24/1, 37 changing color-bar frames, no audio. Extract `1` microsecond, a middle-frame center, the final-frame center, and the two integer microseconds bracketing frame 12's start.
  2. 640x360 at 30000/1001, 61 changing frames with non-block-aligned AAC input. Prove selection uses video timing and accepts the optional exact AAC companion without decoding it.
  3. 1280x720 at 24/1, three frames, exact-size smoke.
  4. Reordered and repeated timestamps to unique paths. Assert result order, path/time echo, exact RGBA length, recomputed SHA-256, alpha `255`, deterministic repeat hashes, and distinct hashes for distinct moving frames.
  5. Compare decoded pixels to the known source pattern with an explicit lossy tolerance (per-channel maximum and aggregate error chosen in the RED test before implementation, tight enough to catch wrong frame/color/range). Label source pixels and decoded pixels separately; never assert source hash equality or attach the encoded-input RGBA as decoded evidence.
  6. Mutated copies of an owned output: truncations at box/sample boundaries and payload, changed brand/profile/dimensions/timescale/`stts`/`ctts`, fragment/edit/external-reference insertion where supported by the mutation helper, sample extent overflow, and a valid but foreign profile. Also mutate actual H.264 bytes for oversized SPS dimensions/reference counts, `avcC`/SPS mismatch, midstream parameter sets, B slices, and extension NALs. Require typed failure, no native decoder create/feed for a whole-file admission failure, and no published frame.
  7. Source/output symlink and escape, preexisting target, aggregate budget one byte short, more than 16 outputs, termination after first progress, deadline exhaustion, and injected/replaced output identity. Require bounded exit/process-group reap and the publication semantics defined above.

  Run:

  ```bash
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/owned-extract-functional --release --functional
  ```

  Expected RED before Task 2; after Task 2 it may be an initial GREEN only if every assertion already executes against the selected release worker. Record which case applies in the SDD task ledger.

- [ ] **Step 2: Make the black-box matrix GREEN without weakening evidence**

  Reuse the harness's structured-argv worker spawn, cleared environment, process-group timeout/cancellation, bounded stdout/stderr, typed transcript validation, and independent MP4 reopen. Generate media incrementally and keep extraction output under the explicit 16-file cap. Do not import the worker crate or `mp4` crate into the functional harness.

  Add a client-contract regression only if these tests reveal a real mismatch. The accepted client already requires positive times; exact ordered path/time/width/height; aggregate budget; and file length/hash identity. Do not edit protocol v1 to make a worker test easier.

- [ ] **Step 3: Run functional plus regression gates and commit**

  Run the Step 1 command, Task 2 worker command, Task 1 host/core command, and Node mutation tests. Expected GREEN: actual encoded MP4s produce actual decoded RGBA results, while all existing encode fixtures stay green.

  ```bash
  rtk /home/olhapi/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin/rustfmt --edition 2024 --check native/linux-permissive-media-worker-functional/src/mp4_reopen.rs native/linux-permissive-media-worker-functional/tests/worker.rs
  rtk git diff --check -- native/linux-permissive-media-worker-functional scripts/build-permissive-media-worker.mjs scripts/build-permissive-media-worker.test.ts
  rtk git add native/linux-permissive-media-worker-functional/src/mp4_reopen.rs native/linux-permissive-media-worker-functional/tests/worker.rs scripts/build-permissive-media-worker.mjs scripts/build-permissive-media-worker.test.ts
  rtk git commit -m "test(linux): qualify owned output frame extraction"
  ```

### Task 4: Produce and review the fresh worker receipt

**Files:** No tracked source is expected. Write only a new ignored evidence root. If receipt tooling lacks a required fact, return to Task 3, add a failing Node test, fix it, and amend through a new Conventional Commit rather than editing accepted evidence.

**Produces:** A fresh immutable development receipt bound to the post-W1 source tree, accepted inputs, host/core and musl tests, black-box process results, compiler/link invocations, Cargo graph, link map, and selected static-musl executable.

- [ ] **Step 1: Run the one final clean evidence build**

  Require at least 8 GiB free. Use a never-before-created root and the accepted inputs without rebuilding or downloading them:

  ```bash
  rtk node --test scripts/build-permissive-media-worker.test.ts
  rtk node scripts/build-permissive-media-worker.mjs --prepared-inputs output/linux-combined-media-proof/inputs/prepared-inputs-task3.json --native-manifest output/linux-combined-media-proof/native-build-attempt4/native-build.json --native-root output/linux-combined-media-proof/native-build-attempt4 --output-root output/linux-permissive-media-worker/owned-output-frame-extraction-final --test-host-core --test-core --test-worker --release --functional --emit-receipt
  ```

  Expected GREEN: every recorded command passes. The selected executable is x86-64 static PIE with no interpreter, `DT_NEEDED`, or undefined OpenH264 symbols. The normal graph contains no Tauri, GTK, WebKitGTK, GStreamer, FFmpeg, LGPL, GPL, or MPL package. The link evidence shows decoder symbols resolved from the exact accepted `libopenh264.a`; source-tree evidence includes every changed bridge/Rust/harness/script file and unchanged source-lock/notices.

- [ ] **Step 2: Review source, license, operation, and nonclaim closure**

  Verify and record:

  - receipt SHA-256 and selected worker SHA-256;
  - the source tree is exactly the reviewed W1 commits and contains no concurrent Desktop files;
  - prepared manifest, native manifest, tool, linker, archive, notice, and Cargo lock identities match the accepted pins;
  - capabilities contain exactly `Capabilities`, `EncodeMp4`, and `ExtractFrames`; encode rows remain exactly eight; `Probe` is absent and typed unsupported;
  - successful files are decoded raw RGBA with recomputed hashes, not copied source/spool bytes;
  - hostile source/path/budget/cancel/deadline/publication cases match the binding behavior, including proof that SPS dimension/reference attacks and every forbidden later AU fail before native allocation/feed;
  - runtime execution has no `PATH`, download, package manager, child codec process, or dynamically loaded codec;
  - the receipt says `development evidence only`, `owned-worker-output-v1 structural subset`, and the nonclaims from this plan.

- [ ] **Step 3: Run final source-only checks and request independent review**

  ```bash
  rtk /home/olhapi/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin/rustfmt --edition 2024 --check src-tauri/crates/permissive-media-worker/src/lib.rs src-tauri/crates/permissive-media-worker/src/job_io.rs src-tauri/crates/permissive-media-worker/src/mp4.rs src-tauri/crates/permissive-media-worker/src/h264_admission.rs src-tauri/crates/permissive-media-worker/src/codec.rs src-tauri/crates/permissive-media-worker/src/color.rs src-tauri/crates/permissive-media-worker/src/capabilities.rs src-tauri/crates/permissive-media-worker/src/worker.rs src-tauri/crates/permissive-media-worker/tests/streaming.rs src-tauri/crates/permissive-media-worker/tests/worker_process.rs native/linux-permissive-media-worker-functional/src/mp4_reopen.rs native/linux-permissive-media-worker-functional/tests/worker.rs
  rtk git diff --check -- src-tauri/crates/permissive-media-worker native/linux-permissive-media-worker-functional scripts/build-permissive-media-worker.mjs scripts/build-permissive-media-worker.test.ts
  rtk git status --short
  ```

  Request a task-spec review, task-quality review, fixes/re-review if needed, then one final W1 review. Do not start Packet 2C routing until root accepts W1 and freezes the new receipt.

## Completion boundary

W1 is complete only when the new worker source, focused tests, black-box actual-decode evidence, independent reviews, and fresh immutable receipt are accepted. It proves bounded frame extraction for the pre-native-admitted fixed worker-output shape and exact raw RGBA result identity. Container labels alone are never sufficient admission. It leaves `Probe`, common/foreign MP4 decode, scaling, Packet 4A, Font Gate A, canonical routing, Settings health, Mac regression, public release authority, and full Linux compatibility open.
