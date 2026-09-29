# Linux Permissive Media Contract and Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fail-closed versioned media protocol and Rust-owned client that can launch only the trusted runtime-manifest media worker, enforce caller-owned project/job boundaries and resource budgets, and reject incompatible or dishonest worker transcripts.

**Architecture:** A dependency-light protocol crate defines bounded request/event types. `PermissiveMediaBackend` resolves the exact worker through `RuntimeResolver`, invokes it only through the existing process supervisor, validates the complete NDJSON transcript and filesystem results, and returns typed results without changing canonical project state. An isolated harness exercises the real library with default features disabled and acts as a fake worker for hostile transcript and lifecycle cases.

**Tech Stack:** Rust 2021, serde/serde_json, static musl test harness, `platform_runtime::RuntimeResolver`, `process_supervisor::run_to_completion`, `RenderCancellationToken`.

**Spec:** [`docs/superpowers/specs/2026-09-12-linux-compatibility-design.md`](../specs/2026-09-12-linux-compatibility-design.md)

**Roadmap:** [`docs/superpowers/plans/2026-09-12-linux-full-compatibility.md`](./2026-09-12-linux-full-compatibility.md), Packet 2 only.

**Prerequisite:** [`docs/superpowers/plans/2026-09-12-linux-runtime-manifest-resolver.md`](./2026-09-12-linux-runtime-manifest-resolver.md) is implemented and reviewed. Use its public resolver API; do not add a second path resolver.

## Global Constraints

- Protocol identity is exactly `video-creater.permissive-media`, schema version 1.
- The only initial encode profile is `H264AacProgressiveBaselineV1`; it is a bounded raw-input contract and must not be described as common MP4 support.
- Production accepts only an artifact returned by `RuntimeResolver::resolve_protocol`; no `PATH`, environment override, arbitrary configured path, download, package manager, GStreamer, FFmpeg, or fallback.
- The caller creates and owns the canonical project render job root and worker output root. This client never chooses, creates, deletes, cleans, or quarantines those roots.
- The client never mutates canonical project files or changes render routing, Settings readiness, export-profile availability, default features, Tauri configuration, or macOS backend selection.
- Every path is validated before launch and revalidated after exit. Symlinks, path escape, stale targets, changed inputs, and worker-returned absolute paths fail closed.
- Reuse `process_supervisor::run_to_completion` and its process-group cleanup. Add no bespoke child kill/reaping implementation.
- Tests run through an isolated harness path dependency on `video-creater` with `default-features = false`; root `src-tauri` integration tests are excluded because its unconditional Tauri dev-dependency can pull GTK.

## Exact protocol contract

Create in `video-creater-permissive-media-protocol`:

```rust
pub const PERMISSIVE_MEDIA_PROTOCOL: &str = "video-creater.permissive-media";
pub const PERMISSIVE_MEDIA_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaRequest {
    pub protocol: String,
    pub schema_version: u32,
    pub request_id: String,
    pub operation: MediaOperation,
    pub budgets: MediaBudgets,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaBudgets {
    pub max_wall_time_ms: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_video_frames: u64,
    pub max_pixels_per_frame: u64,
    pub max_audio_frames: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaProtocolLimits {
    pub max_wall_time_ms: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_video_frames: u64,
    pub max_pixels_per_frame: u64,
    pub max_audio_frames: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum MediaOperation {
    Capabilities,
    EncodeMp4 {
        caller_job_root: String,
        output_root: String,
        output_path: String,
        video: RawVideoInput,
        audio: Option<RawAudioInput>,
        profile: MediaOutputProfile,
    },
    Probe { caller_job_root: String, source_path: String },
    ExtractFrames {
        caller_job_root: String,
        source_path: String,
        output_root: String,
        width: u32,
        height: u32,
        frames: Vec<FrameExtraction>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RationalFrameRate { pub numerator: u32, pub denominator: u32 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawVideoInput {
    pub path: String,
    pub pixel_format: RawPixelFormat,
    pub width: u32,
    pub height: u32,
    pub frame_rate: RationalFrameRate,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawAudioInput {
    pub path: String,
    pub sample_format: RawSampleFormat,
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RawPixelFormat { Rgba8 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RawSampleFormat { S16leInterleaved }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "profile", rename_all = "camelCase", deny_unknown_fields)]
pub enum MediaOutputProfile {
    H264AacProgressiveBaselineV1 {
        video_bitrate: u32,
        keyframe_interval_frames: u32,
        audio_bitrate: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameExtraction { pub presentation_time_us: u64, pub relative_output_path: String }
```

All six budgets and every numeric operation field are nonzero. `MediaRequest::validate(&MediaProtocolLimits)` uses limits supplied by the caller/preflight; the protocol invents no product-size defaults. It uses checked multiplication to prove exact raw byte counts fit `max_input_bytes`, frames fit `max_video_frames`/`max_audio_frames`, and `width * height` fits `max_pixels_per_frame`. `Capabilities` still requires nonzero budgets so every request has one envelope rule.

Events repeat protocol, schema version, and request ID:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "camelCase", deny_unknown_fields)]
pub enum MediaEvent {
    Started { protocol: String, schema_version: u32, request_id: String },
    Progress { protocol: String, schema_version: u32, request_id: String, completed: u64, total: u64 },
    Completed { protocol: String, schema_version: u32, request_id: String, result: MediaResult },
    Failed { protocol: String, schema_version: u32, request_id: String, error: MediaWorkerError },
}
```

The result vocabulary is also closed and typed:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaCapabilities {
    pub operations: Vec<MediaOperationKind>,
    pub encode: Vec<EncodeCapability>,
    pub limits: MediaProtocolLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncodeCapability {
    pub profiles: Vec<MediaOutputProfile>,
    pub pixel_format: RawPixelFormat,
    pub sample_format: Option<RawSampleFormat>,
    pub min_width: u32,
    pub max_width: u32,
    pub width_multiple: u32,
    pub min_height: u32,
    pub max_height: u32,
    pub height_multiple: u32,
    pub frame_rates: Vec<RationalFrameRate>,
    pub sample_rates: Vec<u32>,
    pub channels: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncodeMp4Result {
    pub relative_output_path: String,
    pub byte_length: u64,
    pub sha256: String,
    pub probe: OwnedMediaProbe,
    pub coded_video_duration_us: u64,
    pub coded_audio_duration_us: Option<u64>,
    pub encoder_delay_audio_frames: Option<u64>,
    pub encoder_padding_audio_frames: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedMediaProbe {
    pub container: OwnedContainer,
    pub byte_length: u64,
    pub duration_us: u64,
    pub video: Option<OwnedVideoProbe>,
    pub audio: Option<OwnedAudioProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedVideoProbe {
    pub codec: OwnedVideoCodec,
    pub width: u32,
    pub height: u32,
    pub frame_rate: RationalFrameRate,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedAudioProbe {
    pub codec: OwnedAudioCodec,
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtractedFrame {
    pub relative_output_path: String,
    pub width: u32,
    pub height: u32,
    pub presentation_time_us: u64,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtractFramesResult { pub frames: Vec<ExtractedFrame> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaWorkerError { pub code: String, pub message: String }
```

For owned audio, `OwnedAudioProbe.frame_count` is the coded PCM-equivalent frame count derived from MP4 sample durations, not the source-input frame count. `coded_audio_duration_us` is required when audio is present and equals `floor(frame_count * 1_000_000 / sample_rate)` with checked wide arithmetic. `OwnedMediaProbe.duration_us` is the maximum coded track duration. Encoder delay and padding are either both `Some` with directly established values or both `None`; unknown values are never serialized as zero. These fields describe coded timing only and do not by themselves establish full-tail recovery, gapless playback, or general A/V synchronization.

`MediaOperationKind`, `MediaOutputProfileKind`, `OwnedContainer`, `OwnedVideoCodec`, and `OwnedAudioCodec` are closed serde enums whose only v1 variants are the corresponding operations, `H264AacProgressiveBaselineV1`, `Mp4`, `H264Baseline`, and `AacLc`. `MediaResult` is a closed tagged enum with `Capabilities(MediaCapabilities)`, `EncodeMp4(EncodeMp4Result)`, `Probe(OwnedMediaProbe)`, and `ExtractFrames(ExtractFramesResult)`. Absolute result paths are structurally invalid.

Capability validation rejects duplicate rows and duplicate profiles within a row. `profiles` is nonempty and carries the full correlated bitrate/keyframe tuple; a profile-kind-only claim is invalid. `width_multiple` and `height_multiple` are nonzero, each inclusive range admits at least one aligned value, and a requested dimension must be both in range and divisible by the corresponding multiple. Clients match the complete requested profile value, pixel/sample formats, aligned dimensions, frame rate, sample rate, and channel count. They never infer support from a protocol version or from maxima alone. A video-only row uses `sample_format: None` with empty sample-rate/channel lists; an audio row uses `Some(S16leInterleaved)` with nonempty lists.

## Exact client contract

Create in `render_pipeline::permissive_media_backend`:

```rust
pub const PERMISSIVE_MEDIA_WORKER_ARTIFACT_ID: &str = "permissive-media-worker";
pub const MEDIA_STDOUT_LIMIT: usize = 1024 * 1024;
pub const MEDIA_STDERR_LIMIT: usize = 128 * 1024;

pub struct PermissiveMediaBackend<'a> { resolver: &'a RuntimeResolver }

pub struct PermissiveMediaRun<T> {
    pub result: T,
    pub events: Vec<MediaEvent>,
    pub process_output: SupervisedOutput,
}

#[derive(Clone, Copy)]
pub struct MediaJobBoundary<'a> {
    pub project_root: &'a Path,
    pub caller_job_root: &'a Path,
    pub output_root: &'a Path,
}

impl<'a> PermissiveMediaBackend<'a> {
    pub fn from_runtime(resolver: &'a RuntimeResolver) -> Result<Self, PermissiveMediaClientError>;
    pub fn capabilities(&self, boundary: &MediaJobBoundary<'_>, request_id: &str, budgets: MediaBudgets, limits: &MediaProtocolLimits, parent_deadline: Duration, cancellation: Option<&RenderCancellationToken>) -> Result<PermissiveMediaRun<MediaCapabilities>, PermissiveMediaClientError>;
    pub fn encode_mp4(&self, boundary: &MediaJobBoundary<'_>, request: MediaRequest, limits: &MediaProtocolLimits, parent_deadline: Duration, cancellation: Option<&RenderCancellationToken>) -> Result<PermissiveMediaRun<EncodeMp4Result>, PermissiveMediaClientError>;
    pub fn probe_owned_mp4(&self, boundary: &MediaJobBoundary<'_>, request: MediaRequest, limits: &MediaProtocolLimits, parent_deadline: Duration, cancellation: Option<&RenderCancellationToken>) -> Result<PermissiveMediaRun<OwnedMediaProbe>, PermissiveMediaClientError>;
    pub fn extract_owned_mp4_frames(&self, boundary: &MediaJobBoundary<'_>, request: MediaRequest, limits: &MediaProtocolLimits, parent_deadline: Duration, cancellation: Option<&RenderCancellationToken>) -> Result<PermissiveMediaRun<ExtractFramesResult>, PermissiveMediaClientError>;
}
```

`from_runtime` performs an initial `resolver.resolve_protocol(PERMISSIVE_MEDIA_WORKER_ARTIFACT_ID, PERMISSIVE_MEDIA_PROTOCOL, 1)` check. Every subprocess launch resolves again immediately before execution so a stale `ResolvedRuntimeArtifact` cannot bypass current hash, path, target, dependency-closure, notice-closure, or install-root permission checks. Keep any constructor that accepts a raw path private under `cfg(test)`; production public API cannot inject a path.

Stable client error codes cover `resolver`, `requestInvalid`, `boundaryInvalid`, `inputChanged`, `outputExists`, `unsupported`, `launch`, `timeout`, `cancelled`, `stdoutTooLarge`, `stderrTooLarge`, `transcriptInvalid`, `workerFailed`, `statusMismatch`, `resultInvalid`, and `outputChanged` under prefix `render.permissiveMedia.`.

## Task 1: Implement and verify the protocol/client seam

**Files:**

- Create `src-tauri/crates/permissive-media-protocol/Cargo.toml`
- Create `src-tauri/crates/permissive-media-protocol/src/lib.rs`
- Create `src-tauri/src/render_pipeline/permissive_media_backend.rs`
- Modify `src-tauri/src/render_pipeline/mod.rs`
- Modify `src-tauri/Cargo.toml` only for the workspace member and path dependency
- Modify `src-tauri/Cargo.lock` only as generated for that workspace/path dependency
- Create `native/linux-permissive-media-contract/Cargo.toml`
- Create `native/linux-permissive-media-contract/Cargo.lock`
- Create `native/linux-permissive-media-contract/src/main.rs`
- Create `native/linux-permissive-media-contract/tests/contract.rs`

**Produces:** Protocol v1 types/validation and a production client that is unusable until a matching runtime-manifest worker exists. It creates no readiness or backend-selection claim.

- [ ] **Step 1: Write protocol RED tests in the protocol crate**

Create the crate manifest and failing unit-test skeleton, then test exact JSON round trips and one-field mutations for every request/operation/event/result. Require `deny_unknown_fields`, exact identity, nonempty request ID, nonzero numerics, checked raw-size arithmetic, caller-supplied limits, relative result paths, lowercase 64-character SHA-256, `Started`/`Progress` counters, and typed worker failures.

For encode results, test coded audio frame/duration consistency, maximum-track probe duration, audio-field presence, and the paired delay/padding rule. Reject source-frame counts presented as coded counts, rounded-up microsecond durations, and only one delay/padding value. Use `None`/`None` for unknown metadata; paired zero values remain representable only when a worker can truthfully establish zero delay and zero padding.

Run:

```bash
rtk cargo test --manifest-path src-tauri/crates/permissive-media-protocol/Cargo.toml
```

Expected: RED because the tested types/validation do not exist. Use the generated lockfile and add `--locked` to subsequent runs.

- [ ] **Step 2: Implement minimal protocol types and validation**

Use serde only plus the standard library. Do not depend on the root app, codec, container, image, audio, process, or platform crates. Ensure parsing and validation are separate: serde rejects shape/unknown fields; `validate` rejects semantic/range/path errors.

Run:

```bash
rtk cargo test --manifest-path src-tauri/crates/permissive-media-protocol/Cargo.toml --locked
```

Expected: all protocol tests pass.

- [ ] **Step 3: Scaffold the isolated RED client harness**

Give `native/linux-permissive-media-contract` its own empty `[workspace]` and lock. Path-depend on `../../src-tauri` with `default-features = false` and on `../../src-tauri/crates/permissive-media-protocol`. The test binary doubles as fake worker when its executable filename is `video-creater-permissive-media-worker`; hostile behavior is selected from the request fixture/request ID, never a production environment variable.

Import `PermissiveMediaBackend` in `tests/contract.rs` and run:

```bash
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked
```

Expected: RED because the backend module/client does not exist.

- [ ] **Step 4: Implement resolver-only construction and job-boundary validation**

Build a private install fixture with mode `0o755`, copy/hardlink the harness executable to `bin/video-creater-permissive-media-worker`, hash it into a valid runtime manifest, and construct the reviewed `RuntimeResolver`. Test that wrong ID/protocol/version/hash/permissions/path fail in the resolver before media code runs.

For operation boundaries, require:

```text
project_root is existing, canonical, regular directory
caller_job_root is existing, canonical, not symlinked, and exactly beneath project_root/renders/<safe-job-segment>
output_root is existing, canonical, not symlinked, and beneath caller_job_root/worker
all input files are existing canonical regular files beneath caller_job_root
all requested outputs are relative normal-component paths beneath output_root and do not exist
```

Call existing `validate_split_project_write_path` for every project-relative path and add stricter canonical containment checks. Snapshot input length/SHA-256 before launch and compare after exit. The backend does not create/delete either root.

- [ ] **Step 5: Implement the supervised invocation exactly once**

Serialize one validated request plus newline into `SupervisedCommand.stdin`. Set:

```rust
SupervisedCommand {
    program: freshly_resolved_worker.path.clone(),
    args: Vec::new(),
    cwd: Some(boundary.caller_job_root.to_path_buf()),
    clear_env: true,
    env: prevalidated_tmpdir_env_or_empty,
    stdin,
    stdout_limit: MEDIA_STDOUT_LIMIT,
    stderr_limit: MEDIA_STDERR_LIMIT,
}
```

`prevalidated_tmpdir_env_or_empty` is empty unless the caller already created a canonical nonsymlink directory beneath `caller_job_root`; in that case its only entry is `TMPDIR`. Call `run_to_completion` with the existing cancellation token as `CancellationSignal`. Treat `parent_deadline` as one absolute operation deadline: the capability check and requested operation share its remaining time, and each supervisor deadline is also bounded by request `max_wall_time_ms`. Other supervision timings reuse existing safe defaults. Do not write new signal/process-group code.

- [ ] **Step 6: Parse and validate the complete transcript**

Reject truncated stdout/stderr, invalid UTF-8, blank/malformed/oversize NDJSON, wrong identity, event before `Started`, duplicate `Started`, progress after terminal, progress completed greater than total, missing terminal, multiple terminal events, bytes after terminal, result type not matching operation, `Completed` with nonzero exit, `Failed` with zero exit, and zero exit without `Completed`.

For encode, probe, and extract, first run `Capabilities` through the same freshly resolved worker and require the exact operation/full-profile-value/format/aligned-dimension/rate/channel tuple before launching the requested operation. Omission means `unsupported`; never infer support from protocol version. The two launches share the caller's absolute parent deadline.

- [ ] **Step 7: Revalidate filesystem results**

After exit, revalidate roots and inputs, then resolve every worker-returned relative path below `output_root` using no symlink components. Require fresh regular files, no preexisting target, exact result byte length/SHA-256, output budget compliance, and probe/duration fields consistent with the operation request. Never trust or return a worker absolute path.

- [ ] **Step 8: Complete the hostile fake-worker matrix**

The isolated harness must cover:

1. Exact capabilities round trip; duplicate or empty profiles; missing full-profile preset; zero or impossible alignment; misaligned dimensions; invalid audio-less/audio-present rows; and a requested bitrate, key interval, sample rate, or channel count absent from the matched row.
2. Valid encode only when a new in-root output matches byte/hash/probe/duration claims.
3. Preexisting target, root outside canonical render job, `..`, absolute result, symlink escape, input mutation, and root replacement.
4. Wrong protocol/version/request ID, event before start, duplicate start/terminal, trailing output, malformed JSON, oversized stdout/stderr, zero without completed, nonzero after completed, and result/operation mismatch.
5. Every input/output/frame/pixel/audio/wall budget at the client boundary, including checked-overflow cases.
6. Hung worker timeout and cancelled worker with a spawned descendant. Observe descendant reaping through a marker/pipe closure and the existing supervisor; add no cleanup implementation.

- [ ] **Step 9: Run the focused gate**

```bash
rtk cargo test --manifest-path src-tauri/crates/permissive-media-protocol/Cargo.toml --locked
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked
rtk cargo test --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked process_supervisor
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
rtk cargo tree --manifest-path native/linux-permissive-media-contract/Cargo.toml --locked --edges normal
```

Expected: all tests/format checks pass; the normal graph contains the real app with no active default features and no Tauri/GTK/WebKitGTK/GStreamer/Core ML/Temporal/wgpu packages. Do not run native codecs or the expensive combined proof.

- [ ] **Step 10: Review scope and commit**

Confirm no edits to `project_export.rs`, export profiles, Settings/readiness, default features, Tauri config, macOS build scripts, combined-proof sources/evidence, or canonical project state. Commit:

```bash
rtk git add src-tauri/crates/permissive-media-protocol src-tauri/src/render_pipeline/permissive_media_backend.rs src-tauri/src/render_pipeline/mod.rs src-tauri/Cargo.toml src-tauri/Cargo.lock native/linux-permissive-media-contract
rtk git commit -m "feat(linux): add permissive media worker contract"
```

## Completion boundary

Passing proves a bounded protocol and trusted, supervised, path-safe client against a hostile fake worker. It does not build or qualify a codec worker, encode real media, route a canonical render, change capability readiness, prove common MP4, or establish Linux release compatibility. The next task is roadmap Packet 2A: implement the static-musl worker against this exact contract with fresh source/contributor/license evidence.
