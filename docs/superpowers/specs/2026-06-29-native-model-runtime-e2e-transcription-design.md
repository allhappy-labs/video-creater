# Native Model Runtime E2E Transcription Design

## Goal

Close the transcription workflow gaps so `VideoCreaterTranscribeMediaWorkflow` can run a real end-to-end transcription on macOS, while keeping Temporal and app-facing interfaces generic enough for future transcription, image generation, and video generation runtimes.

The first concrete runtime is a native macOS FluidAudio/Core ML adapter for Parakeet. That adapter is an implementation detail behind a replaceable model-runtime boundary. Temporal should not know Parakeet bundle names, FluidAudio repository names, or Core ML graph names.

## Context

The current transcription path has the right workflow shape but still stops before real inference:

- `ProbeMedia` resolves media, global model status, and runtime metadata.
- `RunTranscription` calls a Rust `TranscriptionBackend`.
- `StoreTranscript` validates a raw transcript artifact and writes canonical project transcript data.
- The Core ML backend validates bundle directories and `video-creater-coreml-inspection.json`, then returns the placeholder error `Core ML Parakeet decode loop is awaiting model IO wiring`.

The earlier design assumed a Rust-owned Parakeet TDT decode loop. That is possible but too large for the next slice: it requires audio preprocessing, model-chain execution, TDT decoder state, blank/duration handling, tokenizer mapping, and timestamp grouping. VoiceInk uses FluidAudio for Parakeet, and FluidAudio already exposes native Swift APIs for loading local Core ML models and returning token/word timings. The fastest path to real E2E behavior is to call a small macOS native helper from Rust.

## Design Principles

- Temporal activity contracts stay model-agnostic.
- Model ids, model bundle layouts, and provider quirks live in catalog metadata and runtime adapters, not in workflow interfaces.
- Rust owns canonical project state, artifact validation, model status, registry selection, and error mapping.
- Native helper processes are adapters. They return typed JSON and never mutate project files.
- Global model storage is shared across all projects.
- No Python or Transformers runtime is introduced.

## Generic Runtime Boundary

Introduce or evolve the runtime boundary around generic model runtime concepts:

```rust
pub enum ModelModality {
    Transcription,
    ImageGeneration,
    VideoGeneration,
}

pub trait ModelRuntime {
    fn runtime_id(&self) -> &'static str;
    fn modality(&self) -> ModelModality;
    fn supports(&self, model: &InstalledModel) -> bool;
    fn probe(&self, model: &InstalledModel) -> RuntimeCapability;
}

pub trait TranscriptionRuntime: ModelRuntime {
    fn transcribe(
        &self,
        job: TranscriptionRuntimeJob,
        model: InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, RuntimeError>;
}
```

`InstalledModel` is the resolved model view from the global model store. It contains at least:

- `model_id`
- `model_dir`
- `runtime_family` or equivalent adapter metadata
- `artifact_format`
- `manifest`
- optional `inspection_report`

The first runtime implementation is:

```text
runtimeId: fluid_audio_coreml
modality: transcription
artifact format: coreml_bundle
adapter: macOS Swift helper using FluidAudio
```

Existing names such as `core_ml_parakeet` may remain only as backward-compatible artifact metadata during migration. New workflow/runtime selection uses runtime ids that describe the implementation family, not a single model.

## Temporal Contract

Temporal `RunTranscription` should receive generic resolved model/runtime fields:

```json
{
  "status": "ready",
  "projectId": "project-1",
  "projectDir": "/path/to/project",
  "mediaId": "media-1",
  "jobId": "transcribe-1",
  "languageMode": "en",
  "sourcePath": "/path/to/media.wav",
  "artifactPath": "/path/to/transcript.json",
  "mediaKind": "audio",
  "mediaDurationSeconds": 12.3,
  "modelId": "nvidia/parakeet-tdt-0.6b-v3",
  "modelPath": "/global/models/nvidia__parakeet-tdt-0.6b-v3",
  "runtimeId": "fluid_audio_coreml"
}
```

Temporal must not branch on the model id or on concrete bundle names. It should:

1. Decode the probe output.
2. Resolve the runtime from `TranscriptionRuntimeRegistry`.
3. Ask the runtime to verify support for the resolved installed model.
4. Call `runtime.transcribe(job, model)`.
5. Write and validate the raw transcript artifact.

Future image/video generation workflows should be able to reuse the same registry pattern with modality-specific runtime traits.

## FluidAudio Adapter

Add a macOS-only native helper, built with SwiftPM and pinned to a specific FluidAudio commit revision in `Package.resolved`. The helper receives one JSON request on stdin or as a request file path and writes one JSON response to stdout.

Request shape:

```json
{
  "schemaVersion": 1,
  "modelId": "nvidia/parakeet-tdt-0.6b-v3",
  "modelPath": "/global/models/nvidia__parakeet-tdt-0.6b-v3",
  "mediaPath": "/path/to/audio-or-video",
  "languageMode": "en"
}
```

Response shape:

```json
{
  "schemaVersion": 1,
  "runtimeId": "fluid_audio_coreml",
  "modelId": "nvidia/parakeet-tdt-0.6b-v3",
  "text": "Hello world",
  "durationSeconds": 1.4,
  "processingSeconds": 0.8,
  "words": [
    {
      "text": "Hello",
      "startSeconds": 0.12,
      "endSeconds": 0.42,
      "confidence": 0.93
    }
  ]
}
```

The helper should load local models only. It must not download models during `RunTranscription`. Download/import remains owned by the app model store.

FluidAudio APIs support local loading through `AsrModels.load(from:version:)` and transcription through `AsrManager.transcribe(...)`, returning `ASRResult` with token timings. Word timings should be derived with FluidAudio's token-to-word timing utilities or equivalent logic in the helper.

## Global Model Layout

The Video Creater model store remains the global source of truth:

```text
~/Library/Application Support/com.olhapi.video-creater/models/
  nvidia__parakeet-tdt-0.6b-v3/
    video-creater-model-manifest.json
    video-creater-coreml-inspection.json
    parakeet-tdt-0.6b-v3-coreml/
      Preprocessor.mlmodelc/
      Encoder.mlmodelc/
      Decoder.mlmodelc/
      JointDecisionv3.mlmodelc/
      parakeet_v3_vocab.json
      config.json
```

FluidAudio expects a repository-style directory for Parakeet v3. The store should normalize imports into this layout or create a compatibility view inside the model directory. Flat imports remain acceptable input, but the installed layout must be deterministic.

The helper receives the model root and adapter metadata tells it where the runtime-specific local model directory is. That keeps repo folder names out of Temporal.

## Artifact Conversion

Rust converts the helper response into the existing raw transcript artifact:

```json
{
  "schemaVersion": 1,
  "mediaId": "media-1",
  "engine": "fluid_audio_coreml",
  "modelId": "nvidia/parakeet-tdt-0.6b-v3",
  "runtimeId": "fluid_audio_coreml",
  "languageMode": "en",
  "tokens": [
    {
      "token": "Hello",
      "start": 0.12,
      "end": 0.42,
      "confidence": 0.93
    }
  ]
}
```

`StoreTranscript` remains responsible for validating and storing canonical project transcript words. Agents then access those words and timings through existing transcript range tools.

## Error Handling

Registry errors:

- unknown runtime id;
- runtime modality mismatch;
- runtime does not support installed model metadata;
- installed model missing required adapter metadata.

Model/layout errors:

- global model missing;
- required runtime-local directory missing;
- required compiled bundle missing;
- inspection report missing or mismatched;
- helper model load failed.

Helper errors:

- invalid JSON request;
- unsupported platform;
- media read/resample failure;
- FluidAudio model load failure;
- FluidAudio transcription failure;
- empty transcript or no usable word timings.

All helper failures map to typed Rust runtime errors. No project transcript is written until the artifact validates.

## Testing And E2E Proof

Unit tests:

- registry selects by `runtimeId` and modality, not model id string branching;
- unsupported runtime and unsupported model metadata fail clearly;
- store normalizes flat imports into the runtime-local layout;
- helper response parser rejects missing/empty words;
- artifact conversion preserves timings and confidence.

macOS integration tests:

- env-gated helper test with `VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT` and `VIDEO_CREATER_TEST_TRANSCRIPTION_WAV`;
- helper returns non-empty words with monotonic timings;
- Rust runtime invokes the helper and produces `TranscriptionRuntimeOutput`.

E2E test:

- create a temporary split project with one media file;
- import or point to a real global model root;
- run `ProbeMedia -> RunTranscription -> StoreTranscript`;
- assert the project contains a transcript for the media id;
- assert transcript words are non-empty and have valid monotonic timings;
- assert `rawArtifactPath` points at the workflow artifact.

The feature is not complete until this E2E test has been run with a real installed model and WAV fixture on macOS.

## Acceptance Criteria

- Temporal activity code depends on `TranscriptionRuntimeRegistry`, not Parakeet-specific runtime classes.
- The first real runtime adapter is `fluid_audio_coreml`, implemented as a macOS-native helper.
- No Python or Transformers dependency is added.
- Global model storage uses deterministic installed metadata and runtime-local layout.
- `RunTranscription` produces a raw transcript artifact with non-empty timed words for a real fixture.
- `StoreTranscript` persists those words into the split project.
- Agents can read the resulting transcript words and timings through the transcript range tool.
- Unsupported platforms and missing models fail before project mutation.
