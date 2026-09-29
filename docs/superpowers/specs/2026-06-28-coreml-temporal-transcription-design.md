# Core ML Temporal Transcription Design

## Summary

Make `VideoCreaterTranscribeMediaWorkflow` end-to-end real in Temporal using a Core ML-first native runtime. Transcription models are global app-managed assets shared by all projects. Temporal remains platform-neutral by calling a replaceable Rust runtime interface; the first supported implementation targets macOS `.mlmodelc` Parakeet artifacts. Python and Transformers are removed from the active runtime selection path.

## Goals

- Replace placeholder Temporal transcription activities with real `ProbeMedia`, `RunTranscription`, and `StoreTranscript` behavior.
- Use global app-managed transcription model storage rather than project-local model artifacts.
- Keep `.mlmodelc` Parakeet bundles as the first active catalog format.
- Add a replaceable native runtime boundary that can support Core ML now and future platform runtimes later.
- Keep canonical project mutation in Rust, through split-project load, validation, and save.
- Fail clearly when the global model is missing, the platform is unsupported, the runtime cannot load the model, or transcript output is invalid.
- Remove Python/Transformers from active runtime selection, setup messaging, and tests for this path.

## Non-Goals

- Shipping a Python or Transformers fallback.
- Making non-macOS transcription work in this slice.
- Implementing an MLX runtime alternative in this slice.
- Mutating project files directly from Temporal sidecars or external scripts.
- Adding a broad model marketplace.
- Adding caption, EDL, or render changes beyond storing a transcript that downstream stages can consume.

## Product Behavior

The app treats transcription models as global application resources. A project can request transcription for a media item, but it does not own or copy the model. The model settings/status surface should point to the global install location and report whether the native macOS runtime is usable.

When a user starts a transcription-backed workflow and no transcript exists for the selected media, Temporal starts `VideoCreaterTranscribeMediaWorkflow`. The workflow checks the media and global model, runs the selected native runtime, stores a raw transcript artifact, then writes the normalized transcript into the split project.

On unsupported platforms, the runtime reports a typed unsupported status. The workflow fails without project mutation and records enough metadata for the UI/job log to explain that macOS Core ML support is required for this implementation.

## Architecture

The implementation should keep Temporal workflow contracts independent from Core ML details:

```text
Temporal workflow input
  -> ProbeMedia activity
  -> RunTranscription activity
  -> TranscriptionRuntime trait
  -> CoreMlParakeetRuntime on macOS
  -> raw transcript artifact
  -> StoreTranscript activity
  -> split project transcript mutation
```

The Rust transcription domain owns model catalog, model path resolution, runtime probing, runtime execution, transcript artifact parsing, and project mutation boundaries.

Primary modules:

- `src-tauri/src/transcription/model.rs`: global `.mlmodelc` catalog entries, required bundle files, manifest validation, safe global model directory names.
- `src-tauri/src/transcription/store.rs`: global model root, installed model status, verification, removal, and active model selection.
- `src-tauri/src/transcription/runtime.rs`: replaceable runtime trait, Core ML runtime implementation slot, unsupported-platform runtime status.
- `src-tauri/src/transcription/job.rs`: Temporal-safe request/output types, artifact paths, runtime errors, and store-transcript validation.
- `src-tauri/src/workflows/mod.rs`: real Temporal activity values for `ProbeMedia`, `RunTranscription`, and `StoreTranscript`.

## Runtime Boundary

Extend the current probe-only runtime interface into an execution boundary similar to:

```rust
pub trait TranscriptionRuntime {
    fn id(&self) -> &'static str;
    fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool;
    fn probe(
        &self,
        model: &TranscriptionModelCatalogEntry,
        artifact_dir: &Path,
    ) -> RuntimeCapability;
    fn transcribe(
        &self,
        job: TranscriptionRuntimeJob,
    ) -> Result<TranscriptArtifact, TranscriptionRuntimeError>;
}
```

`CoreMlParakeetRuntime` is the first active runtime. It supports `.mlmodelc` catalog entries and should only report `Ready` when the platform is macOS and the required global model bundle files are present. Non-macOS builds report `UnsupportedPlatform`, not generic unavailable.

Runtime selection should be deterministic:

1. Select Core ML when the active model supports `.mlmodelc` and the Core ML runtime probes ready.
2. Report unsupported platform when the model is installed but the current platform cannot run the runtime.
3. Report unavailable when model files are missing, invalid, or the runtime cannot initialize.

Python/Transformers runtime types, setup messaging, and tests should be removed from the active app path. If deleting a legacy file causes unrelated churn, it may remain temporarily only when no runtime selection, readiness check, UI message, or workflow activity can reach it.

## Global Model Storage

Models live under an app-managed global model root. The exact root should use the existing Tauri/app data resolution pattern if one exists; otherwise use a single Rust-owned path helper so the location can change without touching workflow code.

Projects store only:

- transcript records in `project.transcripts`
- raw transcript artifact references when useful for logs/review
- workflow/job metadata

Projects must not store model bundles or point to project-local Python artifacts.

The Parakeet catalog remains `.mlmodelc` based. Manifest validation should require the known compiled model bundle files and reject missing, zero-byte, or directory-instead-of-file entries. Verification writes a global manifest, not a project manifest.

## Temporal Workflow

`VideoCreaterTranscribeMediaWorkflow` should become a real workflow:

1. `ProbeMedia`
   - Decode workflow input.
   - Load the split project.
   - Resolve the target media item.
   - Validate the media has an accessible source path and duration metadata, probing if needed.
   - Resolve active global transcription model and runtime status.
   - Return a normalized probe output for the next activity.

2. `RunTranscription`
   - Accept the probe output.
   - Call runtime selection.
   - Run `CoreMlParakeetRuntime::transcribe` through the trait.
   - Write a raw transcript artifact under the project workflow/artifacts area.
   - Return artifact path, media id, engine id, runtime id, language mode, timing summary, and token/word counts.

3. `StoreTranscript`
   - Read and validate the raw transcript artifact.
   - Normalize it into the existing `Transcript`, `TranscriptSegment`, and `TranscriptWord` model.
   - Replace any existing transcript for the same media id.
   - Save the split project atomically through the existing project storage boundary.
   - Return stored transcript id and counts.

If any activity fails before storage, no project transcript changes are written.

## Transcript Artifact Shape

The raw artifact should be explicit and runtime-neutral:

```json
{
  "schemaVersion": 1,
  "mediaId": "media-1",
  "engine": "core_ml_parakeet",
  "modelId": "nvidia/parakeet-tdt-0.6b-v3",
  "runtimeId": "core_ml_parakeet",
  "languageMode": "en",
  "tokens": [
    { "token": "Hello", "start": 0.4, "end": 0.7, "confidence": null }
  ]
}
```

`StoreTranscript` owns converting this artifact into canonical project transcript data. Token cleanup and segment grouping can reuse the existing transcript parsing helpers, adapted away from Parakeet Python-specific naming if needed.

## Error Handling

- Missing global model: fail in `ProbeMedia` with `ModelNotReady`.
- Invalid `.mlmodelc` bundle: fail before inference with `RuntimeUnavailable` or `ModelInvalid`.
- Unsupported platform: fail with `UnsupportedPlatform`.
- Core ML initialization failure: fail `RunTranscription` and preserve logs/artifact metadata.
- Empty or malformed transcript artifact: fail `StoreTranscript` with no partial project save.
- Missing media path: fail `ProbeMedia`.
- Existing transcript for same media id: replace idempotently only after successful artifact validation.

Failures should be represented as typed Rust errors and serialized into Temporal activity failures or structured workflow output. Avoid string-only placeholder statuses for real activity behavior.

## Testing

Use test-driven development. Add failing tests before production edits.

Rust tests should cover:

- Parakeet catalog remains `.mlmodelc` and lists required compiled bundle files.
- Global model path helper resolves outside project directories and uses safe model ids.
- Manifest validation accepts complete nonempty `.mlmodelc` bundles and rejects partial installs.
- Runtime selection reports Core ML ready on macOS when the model is valid.
- Runtime selection reports unsupported platform on non-macOS.
- Python/Transformers cannot satisfy runtime readiness.
- `ProbeMedia` returns normalized media/model/runtime data for a valid project and model.
- `RunTranscription` activity calls the runtime boundary and writes a raw artifact.
- `StoreTranscript` converts a raw artifact into `project.transcripts`.
- `StoreTranscript` replaces an existing transcript for the same media id idempotently.
- Bad artifact input does not mutate the split project.
- Temporal worker manifest still lists `ProbeMedia`, `RunTranscription`, and `StoreTranscript`, but activity tests prove they no longer return placeholder `"registered"` payloads for real transcription inputs.

Frontend tests should be updated only where current UI expects Python runtime availability. The UI should display native/Core ML status and must not instruct users to install Python artifacts for transcription.

## Migration And Cleanup

Remove Python/Transformers references from the active app path in this order:

1. Runtime selection and readiness checks.
2. UI labels and setup guidance.
3. Tests that assert Python fallback readiness.
4. Python runner script and command builder if no remaining tests or tools depend on them.

If deleting the Python runner causes unrelated churn, leave the file temporarily but make it unreachable from app runtime selection and document it as legacy pending removal.

## Acceptance Criteria

- `VideoCreaterTranscribeMediaWorkflow` runs real activities for valid transcription inputs.
- A valid raw transcript artifact is stored and then reflected in `project.transcripts`.
- Global model status is used for every project.
- Python/Transformers do not make transcription ready.
- Core ML is the first selected runtime on macOS with a verified `.mlmodelc` bundle.
- Unsupported platforms fail clearly without pretending transcription is available.
- No activity writes canonical project state until transcript artifact validation succeeds.
- Tests cover the runtime selection, activity behavior, project mutation, and Python fallback removal.
