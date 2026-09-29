# MP4 Export Policy And Temporal Workflow Design

## Context

Palmier documents export support for MP4 formats and NLE XML, while Video Creater currently exposes
Draft WebM, Final WebM, Premiere XML, DaVinci XML, and disabled MP4/H.264, MP4/H.265, and ProRes MOV
actions. This is the right interim state: WebM is the reviewed local render path, NLE XML is a safe
project-file handoff, and MP4-family encoders remain policy-gated until codec provenance, licensing,
runtime availability, and distribution behavior are explicit.

The missing piece is a concrete approval and implementation contract for turning the disabled MP4
actions into real Temporal-backed export jobs without weakening the current local-first and
text-file-editable project model.

References:

- Palmier docs: https://www.palmier.io/docs
- Palmier product page: https://www.palmier.io/
- Temporal Rust SDK guide: https://docs.temporal.io/develop/rust
- Temporal Rust quickstart: https://docs.temporal.io/develop/rust/quickstart
- Existing Video Creater export design:
  `docs/superpowers/specs/2026-06-23-nle-xml-export-design.md`
- Existing GStreamer policy gate:
  `docs/superpowers/specs/2026-06-18-gstreamer-ges-render-backend-design.md`

## Goal

Define the policy, data model, workflow boundaries, and verification gates required before Video
Creater enables MP4/H.264, MP4/H.265, and ProRes MOV export profiles in the editor.

## Non-Goals

- Do not enable any MP4-family export button in this slice.
- Do not add unreviewed encoder elements, ffmpeg arguments, or platform codecs.
- Do not replace the existing WebM render path.
- Do not write provider credentials, API keys, or user secrets into source, tests, logs, reports,
  screenshots, or project files.
- Do not bypass Rust project-action validation or write canonical project files directly from a
  worker.

## Export Profiles

Add an explicit typed export profile model before implementation:

- `mp4H264`: MP4 container, H.264 video, AAC audio when audio is present.
- `mp4H265`: MP4 container, H.265 video, AAC audio when audio is present.
- `proResMov`: MOV container, ProRes video, PCM or AAC audio depending on approved encoder stack.

Each profile must declare:

- file extension and MIME type;
- video encoder family;
- audio encoder family;
- required runtime tools or GStreamer factories;
- licensing and redistribution notes;
- whether the profile is available in the current build;
- install or setup hint when unavailable.

The frontend must keep rendering unavailable profiles as disabled actions until Rust reports the
profile as available.

## Encoder Policy Gate

Before a profile can become available, Rust must expose a reviewed policy report:

```rust
pub struct ExportProfileAvailability {
    pub profile: ExportProfile,
    pub available: bool,
    pub container: String,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub required_runtime: Vec<String>,
    pub policy_status: ExportPolicyStatus,
    pub unavailable_reason: Option<String>,
}
```

Policy statuses:

- `approved`: profile may be enabled.
- `missingRuntime`: profile is approved but local encoder/runtime is missing.
- `policyGated`: profile is intentionally disabled pending approval.
- `unsupportedBuild`: profile is unavailable for the current platform or build feature.

The policy report must be deterministic and free of project or credential data. It may inspect local
runtime capability, but it must not start an export.

## Temporal Workflow Boundary

MP4-family exports should run through Temporal, not direct frontend execution.

Add or extend a render/export workflow with these activities:

1. `BuildRenderPlan`: reuse the canonical timeline, render settings, selected export profile, and
   project-relative output path.
2. `ValidateExportProfile`: fail early if the profile is not `approved` and locally available.
3. `RenderMedia`: invoke the approved backend with bounded command arguments and log capture.
4. `ValidateRenderedMedia`: probe duration, video stream, audio stream when expected, codec family,
   container, dimensions, frame rate, and nonzero file size.
5. `AttachRenderReport`: record command, logs, validation summary, and artifact paths.
6. `WriteExportArtifact`: create the durable project-relative export artifact under `exports/`.
7. `AttachExportReport`: record a `ProjectExportArtifact` with kind `mp4` and the profile format.

The workflow must only apply canonical changes through validated `ProjectAction` values. The worker
may create temporary files and durable artifacts, but it must not directly mutate the project JSON
or split-project files.

## Project File Contract

Extend export artifacts without breaking existing NLE XML records:

- `kind`: existing `mp4` is used for MP4/H.264 and MP4/H.265.
- `kind`: add `mov` only if ProRes MOV cannot be represented by the existing enum safely.
- `format`: one of `mp4H264`, `mp4H265`, `proResMov`.
- `path`: project-relative, under `exports/`.
- `mimeType`: `video/mp4` or `video/quicktime`.
- `jobId`: Temporal job id.
- `createdAt`: ISO timestamp from Rust.

If `mov` requires a schema extension, add a migration and keep older project files loadable.

## UI Behavior

The current export menu remains the right surface:

- Show Draft WebM and Final WebM as available local render actions.
- Show Premiere XML and DaVinci XML when a split project folder is available.
- Show MP4/H.264, MP4/H.265, and ProRes MOV with enabled/disabled state from Rust policy report.
- Disabled profiles show a short reason such as `Policy gated`, `Install approved encoder runtime`,
  or `Unsupported in this build`.
- Enabled profiles start a Temporal export workflow and show the job in the Workflow queue.
- The Workflow queue displays workflow id, task queue, run id, activity list, status, output path,
  and validation result when available.

The UI must not decide codec availability from browser state or hard-coded assumptions.

## Error Handling

Exports fail before rendering when:

- the profile is not approved;
- required runtime is missing;
- output path is unsafe or outside `exports/`;
- timeline validation fails;
- render settings are invalid.

Exports fail after rendering when:

- output file is missing or empty;
- codec/container does not match the selected profile;
- duration is zero or materially different from the render plan;
- video stream is missing;
- audio stream is missing when the timeline contains audible audio;
- log or probe artifacts cannot be written.

Failed workflows must record a failed job status and keep enough log/probe artifact paths for
debugging. They must not record a successful export artifact.

## Testing

Add tests before implementation:

- Rust unit tests for the export profile policy report with all profiles initially `policyGated`.
- Rust tests for profile availability when a mocked approved runtime is present.
- Rust project-action tests for MP4 export artifacts and any required `mov` schema extension.
- Render-plan tests proving MP4 profile selection changes output path, container, and validation
  expectations without changing EDL timeline semantics.
- Frontend tests proving disabled MP4 actions render the Rust-provided reason.
- Frontend tests proving an enabled profile starts a Temporal workflow request and does not directly
  mutate project state.
- End-to-end smoke with a tiny fixture only after an approved runtime exists.

Verification commands for implementation slices:

```bash
rtk pnpm test
rtk pnpm lint
rtk pnpm build
rtk cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

Feature-gated Temporal worker checks also require `protoc` and the Temporal CLI:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml --features temporal-worker --bin video-creater-temporal-worker
```

## Implementation Slices

### Slice 1: Policy Report

Add Rust export profile types and an unavailable-by-default policy report. Surface the report in the
export menu so disabled MP4 actions explain why they are gated.

### Slice 2: Project Artifact Contract

Extend project export artifact validation only as needed for `mp4H264`, `mp4H265`, and `proResMov`
formats. Keep paths project-relative under `exports/`.

### Slice 3: Temporal Start Request

Add an export start request builder for MP4-family profiles. The request records profile, output
path, project id, project directory, job id, and validation expectations, but no credentials.

### Slice 4: Approved Runtime Adapter

After a separate encoder policy review approves a backend, add the adapter behind a feature or
runtime capability gate. Keep unavailable profiles disabled on systems without the runtime.

### Slice 5: Render And Validate

Run the approved adapter through Temporal, probe the output, attach render/export reports, and show
the completed artifact in the inspector.

## Acceptance Criteria

- MP4/H.264, MP4/H.265, and ProRes MOV have explicit policy states instead of hard-coded disabled
  UI.
- No MP4-family profile can be enabled without a Rust policy report marking it approved and locally
  available.
- Enabled MP4-family exports run through Temporal workflow metadata and validated project actions.
- Export artifacts remain text-file-editable and project-relative.
- Failed exports record job failure and diagnostic artifacts without recording successful artifacts.
- MP4 remains disabled until an approved encoder runtime is implemented.

## Spec Self-Review

- No unresolved placeholders remain.
- Scope is limited to policy, contract, workflow boundaries, and staged implementation.
- The spec does not approve any codec runtime by itself.
- The spec preserves the existing WebM and NLE XML paths.
- The spec keeps secrets out of source, project files, logs, tests, and screenshots.
