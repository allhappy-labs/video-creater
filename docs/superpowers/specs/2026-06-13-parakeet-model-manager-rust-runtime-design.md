# Parakeet Model Manager And Rust Runtime Design

## Summary

Add a dedicated Settings / Models screen for installing and managing the Parakeet TDT 0.6B v3 transcription model. Rust owns local model state, model download and verification, transcription job orchestration, runtime selection, logs, cancellation, transcript parsing, and project mutation. The frontend displays model state and starts user actions through Tauri commands.

The implementation creates a stable runtime abstraction so Video Creater can keep the current Python/Transformers Parakeet runner as a fallback while actively developing a Rust-native Parakeet runtime behind the same interface. The app must prefer a capability-proven native runtime when available, but the first shippable stage should not block local transcription on solving native model execution.

## Goals

- Provide a VoiceInk-style local model management screen for Parakeet v3.
- Store model files in an app-managed local directory.
- Show missing, downloading, verifying, ready, and failed model states.
- Gate Generate edit when the local model is not ready.
- Run transcription only through Rust-owned job and runtime orchestration.
- Make the Python runner use Rust-managed local model paths instead of implicitly choosing or downloading by model id.
- Add a `TranscriptionRuntime` abstraction that supports both Python fallback and Rust-native runtime attempts.
- Keep canonical transcript and timeline mutation in Rust.
- Leave the frontend as a thin editor/settings surface.

## Non-Goals

- Removing the Python fallback in the first stage.
- Guaranteeing Rust-native Parakeet inference in the first stage.
- Supporting a broad model marketplace.
- Cloud transcription, hosted model execution, or remote project state.
- Replacing the existing EDL, caption, or render pipeline.
- Building a full preferences system beyond the model settings needed here.

## Product Behavior

The app adds a dedicated Settings / Models screen. The screen includes a Parakeet TDT 0.6B v3 card with:

- install status
- model size and local path summary
- download progress
- verification status
- active runtime preference
- native runtime capability status
- Python fallback availability
- actions for download, cancel, verify, repair or redownload, and remove

When the model is missing or invalid, Generate edit is blocked before transcription starts and offers a direct route to Settings / Models. When the model is ready, Generate edit starts a Rust-owned local transcription job. The editor should not ask the Python sidecar to resolve, download, or mutate model state.

The selected layout is a dedicated Settings / Models screen rather than inline controls in the Codex panel. A first-run setup gate can be added later, but it is not required for the first implementation.

## Architecture

Create a Rust transcription model-management domain under `src-tauri/src/transcription`, with these responsibilities:

- define the Parakeet v3 model catalog entry
- resolve the app-managed model directory
- define the required file manifest
- track download, verification, and removal state
- validate file presence, size, and integrity
- expose runtime capability status
- own transcription job state, logs, cancellation, and artifact paths

The model catalog should include `nvidia/parakeet-tdt-0.6b-v3` and the required local files:

- `model.safetensors`
- `tokenizer.json`
- `tokenizer_config.json`
- `config.json`
- `processor_config.json`
- `generation_config.json`
- `.nemo` only if the fallback path requires it

The model manager should store a local manifest after successful verification. The manifest records model id, source revision if known, installed files, sizes or checksums where available, installed time, verification time, and runtime compatibility metadata.

## Runtime Abstraction

Define a Rust runtime boundary similar to:

```rust
trait TranscriptionRuntime {
    fn id(&self) -> &'static str;
    fn probe(&self, model: &InstalledModel) -> RuntimeCapability;
    fn transcribe(&self, job: TranscriptionJob) -> Result<Transcript, TranscriptionError>;
}
```

The first implementation has two runtimes:

- `PythonParakeetRuntime`: existing Python runner supervised by Rust. It receives local paths from the Rust model manager and cannot choose or download the model implicitly.
- `NativeParakeetRuntime`: Rust-native implementation slot. It starts as unsupported or experimental until it can load model assets and emit transcript timestamps with acceptable parity.

Runtime selection should be deterministic:

1. Prefer Rust-native if the user preference allows it and `probe` reports ready.
2. Fall back to Python when native is unsupported and Python fallback is available.
3. Report runtime unavailable if neither runtime can transcribe with the installed model.

## Tauri Commands

Expose model state and actions through Tauri commands. The Stage 1 command set should include:

- `list_transcription_models`
- `download_transcription_model`
- `cancel_model_download`
- `verify_transcription_model`
- `remove_transcription_model`
- `get_transcription_runtime_status`
- `start_transcription_job`

Generate edit should call into the same Rust model and runtime services before building transcripts, moments, EDLs, captions, layers, or render plans.

## Data Flow

```text
Settings / Models
  -> Rust model catalog
  -> Rust downloader/verifier
  -> installed model manifest
  -> runtime probe
  -> Generate edit
  -> Rust transcription job
  -> selected runtime
  -> normalized transcript JSON
  -> Rust transcript parser
  -> project.transcripts
  -> EDL/caption/timeline pipeline
```

The Python fallback should change from model-id-driven loading to path-driven loading. It should accept a local model directory or local model asset paths supplied by Rust. The runner may still use Transformers internally, but it cannot mutate app model state or silently fetch missing files during normal app transcription.

## Error Handling

- Missing model: block Generate edit and link to Settings / Models.
- Partial download: show repair or resume, and do not allow runtime use.
- Verification failure: mark the model failed and offer redownload.
- Native runtime unsupported: fall back to Python when available.
- Python fallback dependency failure: keep the model installed, but mark fallback runtime unavailable with a log reference.
- Transcription failure: record job error and log path, and do not mutate canonical project transcript or timeline.
- Cancellation: cancel download or transcription and leave project state unchanged.
- Invalid model path: reject paths outside the app-managed model directory.

## Rust-Native Runtime Spike

The native runtime work is a first-class implementation track behind the runtime trait. It should not block Stage 1 app behavior.

The spike should attempt to:

- load tokenizer and processor configuration
- load `model.safetensors`
- preprocess 16 kHz mono audio
- run model generation through a Rust ML runtime if feasible
- decode token durations into word or token timestamps
- compare output against the Python fallback on a tiny fixture

The native runtime remains disabled unless capability tests prove it can load the model and produce timestamps that are usable by the existing transcript-to-EDL and caption pipeline.

## Frontend

React remains frontend-only. It should:

- render Settings / Models
- show model and runtime state returned by Rust
- invoke Tauri commands for model actions
- show progress, failed, cancelled, and ready states
- route users from blocked Generate edit to Settings / Models

The frontend should not:

- construct canonical model manifests
- decide runtime availability
- choose model file paths
- parse transcript artifacts into project state
- mutate canonical project files directly

## Testing

Use test-driven development for the implementation.

Rust tests should cover:

- model catalog entry for Parakeet v3
- app-managed model directory path safety
- manifest validation for complete and partial installs
- download state transitions
- verification success and failure
- runtime selection with native ready, native unsupported with Python fallback, and no runtime available
- Generate edit blocked when the model is not ready
- transcription job failure does not mutate project transcripts or timeline

Frontend tests should cover:

- missing model state renders download action
- downloading state renders progress and cancel action
- failed verification state renders repair or redownload action
- ready state renders runtime status
- Generate edit blocked state links to Settings / Models

Integration tests should cover:

- Python runtime receives local model paths from Rust-managed state
- a tiny transcript fixture becomes project transcript data through the Rust orchestration path
- native runtime probe can return unsupported without breaking fallback transcription

## Staging

### Stage 1: Shippable Model Manager

- Settings / Models screen.
- Rust model catalog and local install state.
- Download, verify, cancel, remove commands.
- Transcription job command that uses the runtime abstraction.
- Generate edit gating.
- Python fallback runtime uses Rust-managed local model path.
- Rust and React tests for all visible state transitions.

### Stage 2: Native Runtime Spike

- Native runtime module behind the runtime trait.
- Model asset loading experiments.
- Audio preprocessing and token decoding experiments.
- Fixture parity comparison against Python fallback.
- Runtime remains disabled unless capability probes pass.

## Acceptance Criteria

- Users can open Settings / Models and manage the Parakeet v3 local model.
- The app distinguishes missing, downloading, verifying, ready, and failed states.
- Model files live under an app-managed directory and are verified before use.
- Generate edit refuses to transcribe without a ready local model.
- Ready models start transcription through Rust-owned job and runtime orchestration.
- The Python runner does not implicitly choose or download model files during normal transcription.
- Runtime selection is owned by Rust and can fall back when native support is unavailable.
- The native runtime slot exists and can report unsupported without breaking local transcription.
- Canonical transcript and timeline mutation remains Rust-owned.
