# Linux Canonical Media Spools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task.

**Goal:** Turn a canonical PNG/WAV `RenderPlan` plus prepared graphics into
bounded raw RGBA8 and 48 kHz stereo S16LE spools that can be passed unchanged to
the Packet 2A permissive media worker.

**Architecture:** Keep project interpretation in the existing Rust render
pipeline. A CPU compositor samples one output frame at a time, reusing the
existing effect, transform, and alpha-compositing primitives. A streaming audio
mixer seeks and resamples supported WAV clips in bounded blocks. An owning guard
keeps uniquely published spool files alive until a later caller finishes with
the worker and then removes only the files it created. Packet 2B constructs and
validates the protocol request in tests; Packet 2C connects it to canonical job
state and the qualified worker.

**Tech Stack:** Rust 2021, existing `image` 0.25 PNG decoder, existing `hound`
3.5 WAV reader, `PreparedEffectStack`, `FrameProgram`,
`transform_rgba8_srgb`, `FrameBlendCompositor`, and
`permissive-media-protocol` schema version 1.

**Spec:**
[`docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`](../specs/2026-09-12-linux-compatibility-design.md)

## Global Constraints

- Complete and review Packet 2 and Packet 2A first. Packet 2A advertises only
  `Capabilities` and `EncodeMp4`, with the eight reviewed output tuples. Packet
  2B must not call `Probe` or `ExtractFrames`, route a canonical job, or claim a
  production backend is ready.
- Do not change a Cargo manifest or lockfile. Do not add a decoder, FFI binding,
  process invocation, `PATH` lookup, FFmpeg, GStreamer, or an LGPL library,
  including an LGPL system library loaded indirectly.
- Preserve macOS default features and routing. Current acceptance runs on this
  Ubuntu VM. Mac execution is deferred to Packet 20 and no Mac result may be
  inferred here.
- Limit this packet to PNG stills, prepared PNG sequences, graphics PNG
  artifacts, and RIFF/WAVE PCM integer 16-bit mono or stereo audio. Every other
  source, container, sample format, channel layout, unsupported effect/property,
  or ambiguous profile fails before any final spool is published.
- Treat the caller-selected `MediaOutputProfile` as an exact capability row.
  Match width, height, rational fps, video bitrate, GOP, audio presence, and AAC
  bitrate. Never rewrite canonical export settings to make a row fit.
- All tests run in `native/linux-permissive-media-contract`, which already
  depends on the real application library with default features disabled. No
  test may synthesize a parallel project/render schema.

## Fixed contracts and rulings

### Public seam

Create this API in
`src-tauri/src/render_pipeline/permissive_compositor.rs` and re-export it only
through `render_pipeline/mod.rs`:

```rust
pub struct PermissiveSpoolBoundary<'a> {
    pub project_root: &'a Path,
    pub caller_job_root: &'a Path,
    pub spool_root: &'a Path,
}

pub struct PreparedMediaSpools { /* owned paths, identities, protocol inputs */ }

impl PreparedMediaSpools {
    pub fn video_input(&self) -> &RawVideoInput;
    pub fn audio_input(&self) -> Option<&RawAudioInput>;
    pub fn cleanup(self) -> PipelineResult<()>;
}

pub fn prepare_permissive_media_spools(
    plan: &RenderPlan,
    graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    boundary: PermissiveSpoolBoundary<'_>,
    profile: &MediaOutputProfile,
    budgets: &MediaBudgets,
    timeout: Duration,
    cancellation: Option<&dyn CancellationSignal>,
) -> PipelineResult<PreparedMediaSpools>;
```

The function derives `RawVideoInput` and optional `RawAudioInput`; it does not
create a `MediaRequest` or invoke a worker. `PreparedMediaSpools` is non-clone.
Its successful lifetime is the ownership boundary Packet 2C will hold around
`encode_mp4`.

Canonicalize all three boundary roots. Require `caller_job_root` to be exactly
`project_root/renders/<one safe segment>` and `spool_root` to be exactly
`caller_job_root/worker`. Reject symlinks and non-directory components. Media
sources must be canonical regular files below `project_root`; graphics artifact
directories and files must be below `caller_job_root`.

Give every invocation unique temporary and final names below the spool root,
for example `video-<uuid>.rgba` and `audio-<same uuid>.s16le`. Create temporary
files with `tempfile` in that directory and publish with
`persist_noclobber`; never truncate or replace a path. Record the final file's
Unix `(device, inode)` immediately after publication. Explicit cleanup and
`Drop` unlink a path only when its current identity still matches. They leave a
caller replacement and every pre-existing file untouched. Temporary-file drop
handles partial writes. Complete and synchronize both temporary spools before
publishing either final path. Publish each file atomically with
`persist_noclobber`; the pair is sequential, and an armed aggregate guard rolls
back the owned video if audio publication or adoption fails.

Use existing `PipelineError` variants with stable paths under
`render.permissiveSpool.{boundary,profile,video,graphics,audio,budget,output,cancelled,timeout,cleanup}`.

### Exact video timeline and composition

Map `plan.fps` only to one of the caller profile's exact rational rows: `24/1`
or `30000/1001`. Accept the normal f64 representation within `1e-9`; reject
`29.97` as an approximation of `30000/1001`. Require the plan dimensions,
audio presence, bitrates, and GOP to equal the selected row.

Normalize finite non-negative canonical seconds to rounded microseconds once.
For output frame `n`, use DTS `n * fps.denominator` at timescale
`fps.numerator`. Determine activity with integer cross-products and half-open
intervals `[start, end)`. The frame count is:

```text
ceil(timeline_duration_us * fps.numerator /
     (1_000_000 * fps.denominator))
```

The maximum clip end defines timeline duration. Each clip must contain a finite
positive `timelineDurationSeconds`. Source clips paint in ascending
`(timeline_track_index, original_render_plan_order)`, so a larger track index is
visually above a smaller one, matching current canonical/GES intent. Prepared
graphics paint after project clips and in the order supplied by the existing
graphics tuple list.

Start every frame as opaque black. For an active visual clip:

1. Decode the exact selected PNG frame as RGBA8. Require its dimensions to
   equal the output canvas in this first slice.
2. Build `PreparedEffectStack` and `FrameProgram` from the canonical properties.
   Sample them at clip-local timeline time.
3. Apply `PreparedEffectStack::apply_rgba8_srgb_at_seconds`.
4. Apply `transform_rgba8_srgb`, including center, size, position, scale,
   rotation, crop, flip, and opacity curves.
5. Composite with `FrameBlendCompositor::new(false)` and the parsed blend mode.

Reject non-empty legacy `colorGrade`, malformed numeric curves, unknown blend
modes, and effect entries that `PreparedEffectStack` does not accept. Do not
silently skip a property. Arbitrary source dimensions/aspect-fit semantics and
ordinary imported-video decode belong to the common-decode packet.

When a clip contains `preparedFrames`, use those PNGs instead of its
`source_path`. Select sequence index
`floor(local_time / frameDurationSeconds)`, capped only at the validated last
frame. Validate that paths, count, duration coverage, dimensions, and alpha
metadata agree before opening the output spool.

For `GraphicsArtifactManifest`, require the current schema version, matching
dimensions/fps, `alpha == true`, safe relative frame paths, and agreement among
`frame_count`, `expected_frames`, `frames_pattern`, `start_number`, and playback
duration. `Sequence` advances by `frame_duration_seconds`; `StaticHold` always
uses its first frame through `duration_seconds`. Outside `[timeline_start,
timeline_start + duration)` it contributes no pixels. Composite every graphics
frame with source-over. Finally flatten the result over opaque black and assert
all output alpha bytes are 255 because Packet 2A ignores alpha.

Hold at most 16 decoded active visual layers plus five canvas-sized work buffers.
Never retain completed output frames or cache every sequence frame. Reject more
than 16 simultaneously active visual layers. Check cancellation and timeout
before each decode, effect, transform, composite, and frame write.

### Bounded audio contract

Create `permissive_audio_mixer.rs` as a private sibling module. Accept only
RIFF/WAVE files that `hound` reports as integer PCM, 16 bits, one or two
channels, and 8,000 through 96,000 Hz. Reject float, compressed, extensible
layouts that do not normalize to that shape, zero-length ranges, non-finite
properties, non-empty audio keyframes, and source ranges outside the WAV data.
Before hound, inspect `fmt ` with a bounded `std` parser. Classic PCM may use
mono or stereo. Extensible PCM must use 16-bit container and valid bits, the
exact PCM subtype, and either mask zero, mono front-center `0x4`, or stereo
front-left/front-right `0x3` with matching channel count; reject other masks,
subtypes, and malformed extents.

The output always begins at timeline sample zero and contains exactly:

```text
ceil(video_frame_count * fps.denominator * 48_000 / fps.numerator)
```

stereo sample frames. If there are no audio clips, omit `audio.s16le` and
require a caller-selected no-audio capability row. If any audio clip exists,
require the matching audio row and write the entire duration, including
zero-valued silence gaps.
Validate clips beyond the video-derived horizon, but clip their contribution at
that horizon; this slice does not preserve an audio-only tail.

For output sample time `t`, an active clip reads source position
`source_in + (t - timeline_start) * speed`. Require source range duration to
match `timelineDurationSeconds * speed` within one source sample. Use linear
interpolation between adjacent PCM frames, duplicate mono to stereo, and retain
only the two source frames needed by each cursor. This Packet 2B speed behavior
changes pitch; pitch-preserving time stretch is explicitly part of later audio
parity.

Convert PCM to f64, apply `10^(volumeDb/20)`, then multiply these half-open
linear envelopes:

```text
fade_in  = min(local_time / fadeInSeconds, 1), or 1 when absent/zero
fade_out = min((duration - local_time) / fadeOutSeconds, 1), or 1 when absent/zero
```

Sum active clips in f64, clamp once to `[-1.0, 32767.0 / 32768.0]`, round to the
nearest signed 16-bit value, and write interleaved little-endian left/right.
Finite non-negative fade durations may overlap; multiply the two envelopes.
There is no normalization, limiter, dithering, or loudness correction in this
slice. Mix in blocks of 1,024 output frames; allow at most 256 clips and 32
simultaneously active audio cursors. Check cancellation, timeout, and byte
budgets at every block.

### Resource limits and qualification boundary

Preflight before creating a temp file. Require nonzero protocol budgets and
enforce `max_pixels_per_frame`, `max_video_frames`, and `max_audio_frames`.
Compute video bytes as `width * height * 4 * video_frame_count` and audio bytes
as `audio_frame_count * 2 channels * 2 bytes`; checked arithmetic and their sum
must fit `max_input_bytes`. Independently cap the aggregate canonical source file
sizes at `max_input_bytes`. The exact spool-byte total is the disk bound. An I/O
or ENOSPC error removes all owned partial/final spools.

The implementation may reuse only this already-resolved dependency closure for
media parsing. The target-normal rows are application runtime source. The five
host-build rows are compiler inputs selected by Cargo feature unification; they
remain source-reviewed under the committed private-development approval but are
not linked into or loaded by the application runtime:

| Package | Selected acceptable license |
| --- | --- |
| `image`, `byteorder-lite`, `bitflags`, `bytemuck`, `cfg-if`, `crc32fast`, `fdeflate`, `flate2`, `miniz_oxide`, `num-traits`, `png` | MIT or Apache-2.0 |
| `moxcms`, `pxfm` | BSD-3-Clause or Apache-2.0 |
| `simd-adler32` | MIT |
| `hound` | Apache-2.0 |
| `adler2` | MIT or Apache-2.0; do not select its alternative 0BSD term |
| `bytemuck_derive` | MIT; host-build proc-macro input |
| `proc-macro2`, `quote`, `syn` | MIT; host-build compiler inputs |
| `unicode-ident` | MIT AND Unicode-3.0; host-build compiler input |

Before implementation acceptance, compare the locked normal graph with this
table, classify every row as target-normal application runtime source or a
host-build compiler input, and inspect each resolved source package's manifest
and license files. A clean comparison means every printed package has an exact
reviewed row and classification; it does not mean host-build inputs are part of
the linked or loaded application runtime.
Block on a new package, native `links` entry, build-time download, unknown
contributor/source provenance, GPL/LGPL term, or mismatch. The existing
declaration-only preflight is supporting evidence, not source qualification.
Packet 2B adds no linked decoder, and its static-musl harness must have no ELF
interpreter or `DT_NEEDED`. Full packaged runtime/SBOM qualification remains a
release gate; do not promote this focused review into that claim.

---

### Task 1: Stream canonical PNG and prepared-graphics frames to RGBA8

**Files:**

- Create: `src-tauri/src/render_pipeline/permissive_compositor.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `native/linux-permissive-media-contract/tests/permissive_compositor.rs`

- [ ] **Step 1: Write failing compositor contract tests**

Add an isolated integration-test target that imports the real root library's
public `render_pipeline::permissive_compositor` seam through the harness's
existing `default-features = false` path dependency. The test target must
contain the following focused tests:

```rust
exact_capability_row_rejects_approximate_or_mismatched_plan
frame_clock_uses_half_open_rational_timeline
higher_track_png_paints_above_lower_track
prepared_effect_transform_and_blend_have_literal_pixels
prepared_clip_sequence_uses_clip_local_time
graphics_sequence_and_static_hold_obey_manifest_schedule
unsupported_visual_property_fails_before_output
escaped_or_malformed_png_inputs_fail_before_output
video_budget_and_active_layer_limits_fail_before_output
cancelled_video_write_removes_only_its_owned_identity
```

Use tiny internal canvas fixtures for pixel semantics and a real supported
640x360 profile for public preflight tests. Assert literal pixels and byte
lengths, no leftover temp files, preservation of a pre-existing path, and
preservation of a replacement installed after the guard recorded its inode.
Define the smallest public compositor types and functions needed by this test
target in Task 1 so Task 2 can consume the seam without relying on private unit
tests. Do not add tests or dev-dependencies to the root Tauri manifest.

- [ ] **Step 2: Run the narrow tests and record the expected RED failure**

Run:

```bash
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --test permissive_compositor -- --test-threads=1
```

Expected: compilation fails because the module and public seam do not exist.

- [ ] **Step 3: Implement the smallest frame writer**

Implement the fixed contracts above. Keep path/profile/timing normalization in
small pure functions, decode only the selected PNG, and use the existing public
frame-compositor APIs for effect, transform, and blend math. Build the video
spool through an internal guard that can be adopted by
`PreparedMediaSpools` in Task 2.

- [ ] **Step 4: Run focused tests GREEN**

Run the Step 2 command. Expected: the named integration target runs 10 tests,
all 10 pass, and the output reports zero filtered tests.

- [ ] **Step 5: Commit the visual slice**

```bash
rtk git add src-tauri/src/render_pipeline/permissive_compositor.rs src-tauri/src/render_pipeline/mod.rs native/linux-permissive-media-contract/tests/permissive_compositor.rs
rtk git commit -m "feat(render): prepare bounded rgba media spools"
```

### Task 2: Mix canonical WAV clips and own the complete spool set

**Files:**

- Create: `src-tauri/src/render_pipeline/permissive_audio_mixer.rs`
- Modify: `src-tauri/src/render_pipeline/permissive_compositor.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `native/linux-permissive-media-contract/tests/permissive_audio_mixer.rs`
- Test: `native/linux-permissive-media-contract/tests/permissive_compositor.rs`

- [ ] **Step 1: Write failing WAV and ownership tests**

Add a second isolated integration-test target that exercises the private mixer
through Task 1's fixed public compositor and spool-ownership seam from the real
root library. Add tests named:

```rust
mono_24khz_ramp_resamples_linearly_to_48khz_stereo
timeline_and_source_trim_with_speed_choose_expected_pcm_frames
gain_and_fades_match_exact_timeline_samples
overlapping_clips_sum_then_saturate_once
timeline_gaps_are_zero_and_output_matches_video_duration
unsupported_wav_shape_and_audio_keyframes_fail_closed
audio_budget_timeout_and_cancellation_remove_both_spools
successful_guard_keeps_spools_until_cleanup
cleanup_does_not_unlink_a_replaced_file
```

Generate minimal PCM WAV fixtures with a small local integration-test helper
using only `std`, so this slice adds no harness or root Cargo dependency. Include
a 24 kHz mono ramp to prove interpolation, a 48 kHz stepped stereo clip with
nonzero `source_in` and speed 2.0, a -6.020599913 dB constant clip with 125 ms
fades, overlapping near-full-scale
clips that clamp to 32767, and silence before/between clips. Use a deterministic
test implementation of the existing `CancellationSignal` trait that flips after
a fixed number of checks.

- [ ] **Step 2: Run the narrow tests and record the expected RED failure**

```bash
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --test permissive_audio_mixer -- --test-threads=1
```

Expected: the target compiles and fails its behavior assertions because the
current public seam returns no audio input or audio spool.

- [ ] **Step 3: Implement streaming audio and aggregate ownership**

Parse and validate every audio clip before publishing either final path. Give
each accepted clip a monotonic two-frame WAV cursor. Mix 1,024-frame blocks using
the fixed formula, write S16LE incrementally, and adopt both published identities
into one `PreparedMediaSpools`. Implement explicit cleanup plus best-effort Drop;
return cleanup failures from the explicit method.

- [ ] **Step 4: Run focused compositor and mixer tests GREEN**

```bash
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --test permissive_compositor -- --test-threads=1
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --test permissive_audio_mixer -- --test-threads=1
```

Expected: the compositor target reports 10 passed with zero filtered and the
mixer target reports 9 passed with zero filtered. The root manifest and both
lockfiles are byte-for-byte unchanged.

- [ ] **Step 5: Commit the complete spool seam**

```bash
rtk git add src-tauri/src/render_pipeline/permissive_audio_mixer.rs src-tauri/src/render_pipeline/permissive_compositor.rs src-tauri/src/render_pipeline/mod.rs native/linux-permissive-media-contract/tests/permissive_audio_mixer.rs native/linux-permissive-media-contract/tests/permissive_compositor.rs
rtk git commit -m "feat(render): mix bounded wav media spools"
```

### Task 3: Prove the saved canonical project feeds the protocol

**Files:**

- Create: `native/linux-permissive-media-contract/tests/canonical_render.rs`

- [ ] **Step 1: Write the failing saved-project fixture**

The test must create this fixture entirely under a temporary canonical project
root:

- A 640x360, 24 fps split project saved by `save_split_project` and reopened by
  `load_split_project`.
- Two distinct 640x360 PNG media items. Cut A occupies `[0, 0.75)` with a
  nonzero source range; cut B occupies `[1.0, 2.0)` with another non-full source
  range and speed 2.0. The gap `[0.75, 1.0)` must render opaque black.
- A real graphics artifact directory under `renders/<job>/graphics`, a manifest
  JSON reopened as `GraphicsArtifactManifest`, and two transparent PNG frames.
  Schedule it over `[0.25, 0.75)` with 250 ms sequence frames. Treat the marker
  as a compact safe-zone callout: restrained high-contrast treatment, one
  deterministic two-state change, clear of the frame edge, and no opaque slab,
  centered template card, or long hold. These are fixture design notes; keep
  the serialized manifest on its existing schema.
- A 24 kHz mono WAV at -6.020599913 dB with 125 ms fades and a nonzero source
  trim, plus a 48 kHz stereo WAV at speed 2.0. Leave audible timeline gaps.

Build the full plan with `build_project_media_render_plan_with_options`. Build a
second `[0.5, 1.25)` plan with
`build_project_provider_input_render_plan_for_range`; assert its first retained
clip starts at zero and source-in advances by 0.5 seconds. Feed both real plans
to `prepare_permissive_media_spools`. Supply the reopened graphics tuple only
to the full plan. Supply no external graphics to the range plan because the
range builder rebases canonical project clips and has no API to rebase an
independent graphics tuple.

Construct a schema-v1 `MediaRequest::EncodeMp4` from the returned inputs and the
explicit 640x360, 24/1, 800,000 bps, GOP 48, AAC 128,000 bps capability row.
Validate it against Packet 2A's reviewed limits. Do not start the worker.

Assert 48 full-render frames, 96,000 audio frames, exact spool byte sizes, cut
pixel changes, black gap pixels, first/second overlay-frame pixels only inside
the scheduled interval, opaque output alpha, exact gain/fade/silence PCM sample
values, and frame-zero rebasing in the range spool. Assert that the selected
source ranges cover less than the media metadata duration, proving this is an
EDL render rather than full-source pass-through.

- [ ] **Step 2: Run the integration test and record the expected RED failure**

```bash
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --test canonical_render -- --test-threads=1
```

Record the actual result. At the reviewed Task 2 source, the expected RED is
`render.permissiveSpool.video` rejecting cut B's canonical static-PNG speed
2.0. If prerequisite work has already made every assertion pass, retain the
honest initial GREEN; do not fabricate a failure.

- [ ] **Step 3: Make only fixture-driven corrections**

Correct compositor/mixer behavior exposed by the real saved project. For the
expected speed failure, accept only finite `0.1..=8.0` speed on a static PNG,
whose sampled pixels do not change and whose exported source range already
represents the retime. Keep prepared PNG sequences restricted to speed 1 until
a separate fixture defines their retiming semantics. Do not add worker
execution, common video decode, project-job mutation, or a profile fallback.
Add a second deterministic cancellation case whose cancellation signal flips
during frame or audio production; after return, assert the spool directory
contains neither a partial file nor any owned final file.

- [ ] **Step 4: Run the focused VM verification and license gates**

```bash
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --test canonical_render -- --test-threads=1
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --target x86_64-unknown-linux-musl -- --test-threads=1
rtk cargo tree --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked -e normal -p image
rtk cargo tree --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked -e normal -p hound
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --target x86_64-unknown-linux-musl --test canonical_render --no-run
```

Inspect the actual emitted musl test executable with `file`, `readelf -l`, and
`readelf -d`; it must have no interpreter and no `DT_NEEDED`. Compare the two
normal dependency trees and every resolved source manifest/license file to the
fixed table above. Treat any unreviewed source or prohibited term as a failed
gate. Confirm with path-scoped diffs that neither Cargo manifest nor lockfile
changed.

- [ ] **Step 5: Commit the canonical fixture**

```bash
rtk git add native/linux-permissive-media-contract/tests/canonical_render.rs src-tauri/src/render_pipeline/permissive_compositor.rs src-tauri/src/render_pipeline/permissive_audio_mixer.rs src-tauri/src/render_pipeline/mod.rs
rtk git commit -m "test(render): verify canonical permissive media spools"
```

## Completion boundary and following work

Packet 2B is complete only when the focused Ubuntu VM tests, cancellation
cleanup, exact request validation, source-license review, and static-musl ELF
checks pass. This proves canonical PNG/WAV/prepared-graphics spool preparation.
It does not prove worker execution, playable MP4 output, common imported-video
decode, pitch-preserving audio, arbitrary image sizing, shared render-job
actions, Linux desktop playback, a physical desktop/audio device, packaged
runtime closure, or macOS behavior.

Packet 2C may then hold `PreparedMediaSpools` across `encode_mp4` and connect it
to shared jobs after the source-qualified Packet 2A worker passes. The common
decode packet must add a separately licensed app-owned normal-import path before
ordinary MP4/WebM sources can enter this compositor. Current VM/headless
acceptance continues without waiting for a real desktop or Mac; those remain
separately labeled release evidence.

Before declaring the implementation plan complete, run:

```bash
rtk rg -n 'TO[D]O|TB[D]|FIX[M]E|implement la[t]er|similar tes[t]s' docs/superpowers/plans/2026-09-12-linux-canonical-media-spools.md
rtk git diff --check -- docs/superpowers/plans/2026-09-12-linux-canonical-media-spools.md
```

The placeholder scan must return no matches.
