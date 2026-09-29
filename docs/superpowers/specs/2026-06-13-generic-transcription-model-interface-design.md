# Generic Transcription Model Interface Design

## Goal

Refactor the transcription model manager so the app treats models as interchangeable transcription engines instead of hardcoding Parakeet into shared interfaces, UI gates, and model manager concepts. Parakeet v3 remains the first catalog entry and first native macOS target, but it should be data and adapter-specific code, not the shape of the system.

## Current Problem

The current implementation has generic pieces with Parakeet-specific names or assumptions:

- `parakeet_v3_catalog_entry()` is the only catalog path.
- `NativeParakeetRuntime` and `PythonParakeetRuntime` sit behind the generic `TranscriptionRuntime` trait.
- UI gate copy says `Parakeet model required.`
- App readiness is a boolean `modelReady`, which hides which model/runtime is selected.
- The model store can list statuses, but there is no explicit active/default transcription model concept.

Parakeet-specific parser and runner code can remain where it represents a concrete adapter. Shared model management, runtime selection, and UI contracts should not assume Parakeet.

## Architecture

Introduce a catalog-first model abstraction:

```rust
TranscriptionModelCatalogEntry {
    id,
    display_name,
    provider,
    family,
    revision,
    artifact_format,
    approximate_size_bytes,
    required_files,
    supported_runtimes,
}
```

`family` is data such as `parakeet`, `whisper`, or a future model family. `artifact_format` is data such as `core_ml_bundle`, `transformers`, `onnx`, or `ggml`. Runtime implementations use those fields to decide support.

The shared Rust API should expose generic model manager operations:

- list transcription models
- get/set active transcription model
- download/import/verify/remove a transcription model by `model_id`
- get runtime status for the active model or a specific `model_id`

The runtime trait should support model-aware probing:

```rust
trait TranscriptionRuntime {
    fn id(&self) -> &'static str;
    fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool;
    fn probe(&self, install: &InstalledTranscriptionModel) -> RuntimeCapability;
}
```

Concrete implementations can still be specific:

- `CoreMlParakeetRuntime` on macOS
- `TransformersPythonRuntime` as a legacy sidecar
- future `OnnxRuntime`, `DirectMlRuntime`, or `WhisperRuntime`

## Data Flow

1. The catalog lists all known transcription models.
2. The store reports install status for each model.
3. The active model id controls Generate edit gating and runtime selection.
4. Runtime selection checks the active model’s artifact format and installed files.
5. UI renders the selected model name from data, while generic copy says `Local transcription model required.`
6. A model-specific runtime adapter turns audio into the common `Transcript` shape.

## UI Scope

Settings / Models should become a generic transcription model picker:

- Show all catalog entries.
- Mark the active model.
- Allow selecting a different active model when installed or available.
- Use generic blocked copy in the editor: `Local transcription model required.`
- Keep model names in data rows and action labels, for example `Download Parakeet TDT 0.6B v3`.

No marketing layout changes are needed. Keep the existing compact editor/settings style.

## Compatibility

Existing commands can remain as wrappers during migration if useful, but new internal names should be model-neutral. Existing Parakeet-specific transcript parsing tests can remain because they test a concrete adapter. The app should avoid user-visible breakage by keeping the current Parakeet catalog entry id stable.

## Error Handling

Unsupported combinations should be explicit:

- model not installed
- active model missing
- artifact format unsupported on this platform
- runtime unavailable
- download/import source not configured

These errors should not mention Parakeet unless the selected model is Parakeet.

## Testing

Add or update tests for:

- catalog supports multiple entries without changing store shape
- active model defaults to the first supported entry
- runtime selection asks runtime support per model/artifact format
- UI blocked state uses generic copy
- Parakeet-specific runtime/parser behavior remains covered as adapter tests

## Out Of Scope For This Refactor

- Implementing the actual Core ML inference loop.
- Adding a second real model.
- Windows runtime support.
- Replacing the transcript JSON schema.

The refactor should prepare those changes without building them yet.
