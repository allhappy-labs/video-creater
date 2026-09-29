# Native Model Runtime E2E Transcription Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Temporal transcription truly end-to-end on macOS through a generic model runtime registry and a native FluidAudio/Core ML transcription adapter.

**Architecture:** Temporal resolves a model/runtime pair through Rust-owned generic registry code and never branches on a concrete model name. The first concrete runtime is `fluid_audio_coreml`, which invokes a small SwiftPM helper pinned to FluidAudio and converts helper word timings into the existing transcript artifact format. Global model storage remains shared across all projects and normalizes Parakeet imports into the FluidAudio-compatible repo layout.

**Tech Stack:** Rust/Tauri, Temporal activity helpers, SwiftPM, FluidAudio, Core ML, serde JSON, macOS env-gated E2E tests.

---

## File Structure

- Modify `src-tauri/src/transcription/model.rs`
  - Add modality/runtime metadata that is generic enough for future model families.
  - Keep Parakeet as catalog data, not workflow logic.
- Modify `src-tauri/src/transcription/store.rs`
  - Normalize imported Core ML bundles into a runtime-local FluidAudio layout.
  - Expose installed model metadata used by runtime selection.
- Modify `src-tauri/src/transcription/runtime.rs`
  - Introduce runtime registry and runtime traits.
  - Keep fixture runtime for tests.
  - Replace the placeholder Core ML backend with registry-based dispatch.
- Create `src-tauri/src/transcription/fluidaudio.rs`
  - Own the helper JSON protocol, helper path resolution, process invocation, response validation, and conversion into `TranscriptionRuntimeOutput`.
- Modify `src-tauri/src/transcription/mod.rs`
  - Export the new `fluidaudio` module.
- Modify `src-tauri/src/workflows/mod.rs`
  - Resolve `RunTranscription` through the registry rather than `CoreMlParakeetBackend`.
  - Preserve artifact validation and project mutation boundaries.
- Create `src-tauri/native/fluidaudio-parakeet/Package.swift`
  - SwiftPM helper package with FluidAudio pinned by commit revision.
- Create `src-tauri/native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribeCore/Protocol.swift`
  - Helper request/response types plus local FluidAudio transcription function.
- Create `src-tauri/native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribe/main.swift`
  - Thin executable entrypoint that accepts JSON, calls the core module, and prints JSON.
- Create `src-tauri/native/fluidaudio-parakeet/Tests/VideoCreaterFluidAudioTranscribeTests/ProtocolTests.swift`
  - Swift protocol tests that do not require a real model.
- Modify `src-tauri/tests/transcription_models.rs`
  - Add registry, installed metadata, import layout, and helper parser tests.
- Modify `src-tauri/tests/temporal_workflows.rs`
  - Update Temporal tests to assert generic registry dispatch.
- Create `src-tauri/tests/transcription_e2e.rs`
  - Env-gated real model E2E test for `ProbeMedia -> RunTranscription -> StoreTranscript`.

---

## Task 1: Generic Runtime Metadata And Registry

**Files:**
- Modify: `src-tauri/src/transcription/model.rs`
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing registry tests**

Add these tests to `src-tauri/tests/transcription_models.rs` near the existing runtime selection tests:

```rust
#[test]
fn runtime_registry_selects_transcription_runtime_by_id_and_modality() {
    let model = parakeet_v3_catalog_entry();
    let installed = video_creater_lib::transcription::runtime::InstalledModel {
        model_id: model.id.to_string(),
        model_dir: std::path::PathBuf::from("/tmp/model"),
        artifact_format: model.artifact_format,
        modality: video_creater_lib::transcription::runtime::ModelModality::Transcription,
        runtime_family: "fluid_audio_coreml".to_string(),
    };
    let registry = video_creater_lib::transcription::runtime::TranscriptionRuntimeRegistry::new(
        vec![Box::new(video_creater_lib::transcription::runtime::FixtureTranscriptionRuntime {
            runtime_id: "fluid_audio_coreml",
            capability: RuntimeCapability::Ready,
            supported: true,
            tokens: vec![TranscriptToken {
                token: "Hello".to_string(),
                start: 0.0,
                end: 0.5,
                confidence: Some(0.9),
            }],
        })],
    );

    let selected = registry
        .select("fluid_audio_coreml", &installed)
        .expect("selected runtime");

    assert_eq!(selected.runtime_id(), "fluid_audio_coreml");
}

#[test]
fn runtime_registry_rejects_transcription_runtime_that_does_not_support_installed_model() {
    let model = parakeet_v3_catalog_entry();
    let installed = video_creater_lib::transcription::runtime::InstalledModel {
        model_id: model.id.to_string(),
        model_dir: std::path::PathBuf::from("/tmp/model"),
        artifact_format: model.artifact_format,
        modality: video_creater_lib::transcription::runtime::ModelModality::Transcription,
        runtime_family: "fluid_audio_coreml".to_string(),
    };
    let registry = video_creater_lib::transcription::runtime::TranscriptionRuntimeRegistry::new(
        vec![Box::new(video_creater_lib::transcription::runtime::FixtureTranscriptionRuntime {
            runtime_id: "fluid_audio_coreml",
            capability: RuntimeCapability::Ready,
            supported: false,
            tokens: Vec::new(),
        })],
    );

    let error = registry
        .select("fluid_audio_coreml", &installed)
        .expect_err("unsupported model should be rejected");

    assert!(error.to_string().contains("does not support"));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models runtime_registry_ -- --nocapture
```

Expected: FAIL because `InstalledModel`, `ModelModality`, `TranscriptionRuntimeRegistry`, and `FixtureTranscriptionRuntime` do not exist yet.

- [ ] **Step 3: Add generic runtime types**

In `src-tauri/src/transcription/runtime.rs`, add:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelModality {
    Transcription,
    ImageGeneration,
    VideoGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModel {
    pub model_id: String,
    pub model_dir: PathBuf,
    pub artifact_format: TranscriptionModelArtifactFormat,
    pub modality: ModelModality,
    pub runtime_family: String,
}

pub trait ModelRuntime: Send + Sync {
    fn runtime_id(&self) -> &'static str;
    fn modality(&self) -> ModelModality;
    fn supports_installed_model(&self, model: &InstalledModel) -> bool;
    fn probe_installed_model(&self, model: &InstalledModel) -> RuntimeCapability;
}

pub trait TranscriptionRuntimeEngine: ModelRuntime {
    fn transcribe_installed(
        &self,
        job: &TranscriptionRuntimeJob,
        model: &InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeRegistryError {
    #[error("unknown transcription runtime {0}")]
    UnknownRuntime(String),
    #[error("runtime {runtime_id} has modality {actual:?}, expected transcription")]
    ModalityMismatch {
        runtime_id: String,
        actual: ModelModality,
    },
    #[error("runtime {runtime_id} does not support model {model_id}")]
    UnsupportedModel {
        runtime_id: String,
        model_id: String,
    },
    #[error("runtime {runtime_id} is not ready: {capability:?}")]
    RuntimeNotReady {
        runtime_id: String,
        capability: RuntimeCapability,
    },
}

pub struct TranscriptionRuntimeRegistry {
    runtimes: Vec<Box<dyn TranscriptionRuntimeEngine>>,
}

impl TranscriptionRuntimeRegistry {
    pub fn new(runtimes: Vec<Box<dyn TranscriptionRuntimeEngine>>) -> Self {
        Self { runtimes }
    }

    pub fn select(
        &self,
        runtime_id: &str,
        model: &InstalledModel,
    ) -> Result<&dyn TranscriptionRuntimeEngine, RuntimeRegistryError> {
        let runtime = self
            .runtimes
            .iter()
            .find(|runtime| runtime.runtime_id() == runtime_id)
            .ok_or_else(|| RuntimeRegistryError::UnknownRuntime(runtime_id.to_string()))?;
        if runtime.modality() != ModelModality::Transcription {
            return Err(RuntimeRegistryError::ModalityMismatch {
                runtime_id: runtime_id.to_string(),
                actual: runtime.modality(),
            });
        }
        if !runtime.supports_installed_model(model) {
            return Err(RuntimeRegistryError::UnsupportedModel {
                runtime_id: runtime_id.to_string(),
                model_id: model.model_id.clone(),
            });
        }
        let capability = runtime.probe_installed_model(model);
        if capability != RuntimeCapability::Ready {
            return Err(RuntimeRegistryError::RuntimeNotReady {
                runtime_id: runtime_id.to_string(),
                capability,
            });
        }
        Ok(runtime.as_ref())
    }
}
```

- [ ] **Step 4: Add fixture runtime implementation**

In `src-tauri/src/transcription/runtime.rs`, add:

```rust
#[derive(Debug, Clone)]
pub struct FixtureTranscriptionRuntime {
    pub runtime_id: &'static str,
    pub capability: RuntimeCapability,
    pub supported: bool,
    pub tokens: Vec<TranscriptToken>,
}

impl ModelRuntime for FixtureTranscriptionRuntime {
    fn runtime_id(&self) -> &'static str {
        self.runtime_id
    }

    fn modality(&self) -> ModelModality {
        ModelModality::Transcription
    }

    fn supports_installed_model(&self, _model: &InstalledModel) -> bool {
        self.supported
    }

    fn probe_installed_model(&self, _model: &InstalledModel) -> RuntimeCapability {
        self.capability.clone()
    }
}

impl TranscriptionRuntimeEngine for FixtureTranscriptionRuntime {
    fn transcribe_installed(
        &self,
        _job: &TranscriptionRuntimeJob,
        model: &InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        Ok(TranscriptionRuntimeOutput {
            runtime_id: self.runtime_id.to_string(),
            model_id: model.model_id.clone(),
            tokens: self.tokens.clone(),
        })
    }
}
```

- [ ] **Step 5: Run tests to verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models runtime_registry_ -- --nocapture
```

Expected: PASS with 2 registry tests passing.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/transcription/runtime.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add generic transcription runtime registry"
```

Expected: commit succeeds.

---

## Task 2: Normalize Global Model Imports For Runtime-Local Layout

**Files:**
- Modify: `src-tauri/src/transcription/model.rs`
- Modify: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing import layout test**

Add this test to `src-tauri/tests/transcription_models.rs` near the existing `store_import_` tests:

```rust
#[test]
fn store_import_normalizes_flat_coreml_bundle_into_fluidaudio_repo_layout() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import flat model");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let installed_root = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    let repo_root = installed_root.join("parakeet-tdt-0.6b-v3-coreml");
    assert!(repo_root.join("Preprocessor.mlmodelc/coremldata.bin").is_file());
    assert!(repo_root.join("Encoder.mlmodelc/coremldata.bin").is_file());
    assert!(repo_root.join("Decoder.mlmodelc/coremldata.bin").is_file());
    assert!(repo_root
        .join("JointDecisionv3.mlmodelc/coremldata.bin")
        .is_file());
    assert!(repo_root.join("parakeet_v3_vocab.json").is_file());
    assert!(installed_root.join(MODEL_MANIFEST_FILE_NAME).is_file());
    assert!(installed_root.join(COREML_INSPECTION_FILE_NAME).is_file());
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_import_normalizes_flat_coreml_bundle_into_fluidaudio_repo_layout -- --nocapture
```

Expected: FAIL because imports currently preserve a flat installed layout.

- [ ] **Step 3: Add runtime-local layout metadata**

In `src-tauri/src/transcription/model.rs`, add:

```rust
pub const FLUID_AUDIO_COREML_RUNTIME_ID: &str = "fluid_audio_coreml";
pub const FLUID_AUDIO_PARAKEET_V3_REPO_DIR: &str = "parakeet-tdt-0.6b-v3-coreml";
```

Add to `TranscriptionModelCatalogEntry`:

```rust
pub runtime_family: &'static str,
pub runtime_model_dir: Option<&'static str>,
```

Set the Parakeet entry:

```rust
runtime_family: FLUID_AUDIO_COREML_RUNTIME_ID,
runtime_model_dir: Some(FLUID_AUDIO_PARAKEET_V3_REPO_DIR),
```

- [ ] **Step 4: Update manifest validation for runtime-local directories**

In `InstalledModelManifest::validate_against` in `src-tauri/src/transcription/model.rs`, replace:

```rust
let model_dir = model_root.join(safe_model_dir_name(entry.id));
```

with:

```rust
let mut model_dir = model_root.join(safe_model_dir_name(entry.id));
if let Some(runtime_model_dir) = entry.runtime_model_dir {
    model_dir = model_dir.join(runtime_model_dir);
}
```

- [ ] **Step 5: Normalize flat imports in the store**

In `src-tauri/src/transcription/store.rs`, add:

```rust
fn runtime_model_dir_for_entry(entry: &TranscriptionModelCatalogEntry, model_dir: &Path) -> PathBuf {
    entry
        .runtime_model_dir
        .map(|runtime_model_dir| model_dir.join(runtime_model_dir))
        .unwrap_or_else(|| model_dir.to_path_buf())
}
```

In `import_model`, after creating `import_model_dir`, copy into the runtime-local directory:

```rust
let import_runtime_model_dir = runtime_model_dir_for_entry(&entry, &import_model_dir);
fs::create_dir_all(&import_runtime_model_dir).map_err(|source| {
    ModelStoreError::CreateImportDir {
        path: import_runtime_model_dir.display().to_string(),
        source,
    }
})?;
```

Update `write_verified_staged_manifest` to validate against the staging root and write the manifest/report at the installed model root, not inside the runtime-local repo directory.

Also add this helper so both flat and already-normalized source directories are accepted:

```rust
fn source_runtime_model_dir_for_entry(
    entry: &TranscriptionModelCatalogEntry,
    source_dir: &Path,
) -> PathBuf {
    if let Some(runtime_model_dir) = entry.runtime_model_dir {
        let nested = source_dir.join(runtime_model_dir);
        if nested.is_dir() {
            return nested;
        }
    }
    source_dir.to_path_buf()
}
```

Use it in `import_model`:

```rust
let source_runtime_model_dir = source_runtime_model_dir_for_entry(&entry, source_dir);
copy_directory_contents(&source_runtime_model_dir, &import_runtime_model_dir)?;
validate_required_import_files(&entry, &import_runtime_model_dir)?;
```

- [ ] **Step 6: Keep source-equals-destination verification in place**

Update `paths_refer_to_same_dir` usage so either of these imported paths verify in place:

```text
<model-root>/nvidia__parakeet-tdt-0.6b-v3
<model-root>/nvidia__parakeet-tdt-0.6b-v3/parakeet-tdt-0.6b-v3-coreml
```

Add this assertion to `store_import_verifies_canonical_equivalent_destination_in_place`:

```rust
assert!(model_dir
    .join("parakeet-tdt-0.6b-v3-coreml/Preprocessor.mlmodelc/coremldata.bin")
    .is_file());
```

- [ ] **Step 7: Run import tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_import -- --nocapture
```

Expected: PASS with all `store_import` tests passing.

- [ ] **Step 8: Commit**

Run:

```bash
rtk git add src-tauri/src/transcription/model.rs src-tauri/src/transcription/store.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: normalize Core ML imports for native runtime layout"
```

Expected: commit succeeds.

---

## Task 3: Add The Native FluidAudio Helper Package

**Files:**
- Create: `src-tauri/native/fluidaudio-parakeet/Package.swift`
- Create: `src-tauri/native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribeCore/Protocol.swift`
- Create: `src-tauri/native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribe/main.swift`
- Create: `src-tauri/native/fluidaudio-parakeet/Tests/VideoCreaterFluidAudioTranscribeTests/ProtocolTests.swift`

- [ ] **Step 1: Create the SwiftPM package**

Create `src-tauri/native/fluidaudio-parakeet/Package.swift`:

```swift
// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "VideoCreaterFluidAudioParakeet",
    platforms: [.macOS(.v14)],
    products: [
        .library(
            name: "VideoCreaterFluidAudioTranscribeCore",
            targets: ["VideoCreaterFluidAudioTranscribeCore"]
        ),
        .executable(
            name: "video-creater-fluidaudio-transcribe",
            targets: ["VideoCreaterFluidAudioTranscribe"]
        )
    ],
    dependencies: [
        .package(
            url: "https://github.com/FluidInference/FluidAudio.git",
            revision: "3c6e79f1d74411cae1f3daf50260dd19a585dc2d"
        )
    ],
    targets: [
        .target(
            name: "VideoCreaterFluidAudioTranscribeCore",
            dependencies: [.product(name: "FluidAudio", package: "FluidAudio")]
        ),
        .executableTarget(
            name: "VideoCreaterFluidAudioTranscribe",
            dependencies: ["VideoCreaterFluidAudioTranscribeCore"]
        ),
        .testTarget(
            name: "VideoCreaterFluidAudioTranscribeTests",
            dependencies: ["VideoCreaterFluidAudioTranscribeCore"]
        )
    ]
)
```

- [ ] **Step 2: Add helper JSON protocol and local-only loading**

Create `src-tauri/native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribeCore/Protocol.swift`:

```swift
import AVFoundation
import FluidAudio
import Foundation

public struct TranscriptionRequest: Codable {
    public let schemaVersion: Int
    public let modelId: String
    public let modelPath: String
    public let mediaPath: String
    public let languageMode: String

    public init(schemaVersion: Int, modelId: String, modelPath: String, mediaPath: String, languageMode: String) {
        self.schemaVersion = schemaVersion
        self.modelId = modelId
        self.modelPath = modelPath
        self.mediaPath = mediaPath
        self.languageMode = languageMode
    }
}

public struct Word: Codable {
    public let text: String
    public let startSeconds: Double
    public let endSeconds: Double
    public let confidence: Double?

    public init(text: String, startSeconds: Double, endSeconds: Double, confidence: Double?) {
        self.text = text
        self.startSeconds = startSeconds
        self.endSeconds = endSeconds
        self.confidence = confidence
    }
}

public struct TranscriptionResponse: Codable {
    public let schemaVersion: Int
    public let runtimeId: String
    public let modelId: String
    public let text: String
    public let durationSeconds: Double
    public let processingSeconds: Double
    public let words: [Word]

    public init(schemaVersion: Int, runtimeId: String, modelId: String, text: String, durationSeconds: Double, processingSeconds: Double, words: [Word]) {
        self.schemaVersion = schemaVersion
        self.runtimeId = runtimeId
        self.modelId = modelId
        self.text = text
        self.durationSeconds = durationSeconds
        self.processingSeconds = processingSeconds
        self.words = words
    }
}

public struct ErrorResponse: Codable {
    public let schemaVersion: Int
    public let error: String

    public init(schemaVersion: Int, error: String) {
        self.schemaVersion = schemaVersion
        self.error = error
    }
}

public func languageHint(_ languageMode: String) -> Language? {
    let trimmed = languageMode.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty, trimmed != "auto" else { return nil }
    return Language(rawValue: trimmed)
}

public func transcribe(_ request: TranscriptionRequest) async throws -> TranscriptionResponse {
    guard request.schemaVersion == 1 else {
        throw NSError(domain: "VideoCreaterFluidAudio", code: 3, userInfo: [
            NSLocalizedDescriptionKey: "unsupported request schemaVersion \(request.schemaVersion)"
        ])
    }

    let modelRoot = URL(fileURLWithPath: request.modelPath, isDirectory: true)
    let repoRoot = modelRoot.appendingPathComponent("parakeet-tdt-0.6b-v3-coreml", isDirectory: true)
    let mediaURL = URL(fileURLWithPath: request.mediaPath, isDirectory: false)

    let started = Date()
    let models = try await AsrModels.load(from: repoRoot, version: .v3)
    let manager = AsrManager(config: .default)
    try await manager.loadModels(models)
    var decoderState = TdtDecoderState.make(decoderLayers: await manager.decoderLayerCount)
    let result = try await manager.transcribe(mediaURL, decoderState: &decoderState, language: languageHint(request.languageMode))
    let wordTimings = buildWordTimings(from: result.tokenTimings ?? [])
    let words = wordTimings.map {
        Word(
            text: $0.word,
            startSeconds: $0.startTime,
            endSeconds: $0.endTime,
            confidence: nil
        )
    }.filter { !$0.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    return TranscriptionResponse(
        schemaVersion: 1,
        runtimeId: "fluid_audio_coreml",
        modelId: request.modelId,
        text: result.text,
        durationSeconds: result.duration,
        processingSeconds: Date().timeIntervalSince(started),
        words: words
    )
}
```

- [ ] **Step 3: Add executable entrypoint**

Create `src-tauri/native/fluidaudio-parakeet/Sources/VideoCreaterFluidAudioTranscribe/main.swift`:

```swift
import Foundation
import VideoCreaterFluidAudioTranscribeCore

func readRequest() throws -> TranscriptionRequest {
    let data = FileHandle.standardInput.readDataToEndOfFile()
    guard !data.isEmpty else {
        throw NSError(domain: "VideoCreaterFluidAudio", code: 2, userInfo: [
            NSLocalizedDescriptionKey: "missing JSON request on stdin"
        ])
    }
    return try JSONDecoder().decode(TranscriptionRequest.self, from: data)
}

@main
struct VideoCreaterFluidAudioTranscribe {
    static func main() async {
        do {
            let request = try readRequest()
            let response = try await transcribe(request)
            let data = try JSONEncoder().encode(response)
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data([0x0a]))
        } catch {
            let response = ErrorResponse(schemaVersion: 1, error: error.localizedDescription)
            let data = (try? JSONEncoder().encode(response)) ?? Data("{\"schemaVersion\":1,\"error\":\"unknown error\"}".utf8)
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data([0x0a]))
            exit(1)
        }
    }
}
```

- [ ] **Step 4: Add a protocol compile test**

Create `src-tauri/native/fluidaudio-parakeet/Tests/VideoCreaterFluidAudioTranscribeTests/ProtocolTests.swift`:

```swift
import Testing
@testable import VideoCreaterFluidAudioTranscribeCore

@Test func responseEncodesRuntimeAndWords() throws {
    let response = TranscriptionResponse(
        schemaVersion: 1,
        runtimeId: "fluid_audio_coreml",
        modelId: "nvidia/parakeet-tdt-0.6b-v3",
        text: "Hello world",
        durationSeconds: 1.0,
        processingSeconds: 0.5,
        words: [
            Word(text: "Hello", startSeconds: 0.0, endSeconds: 0.4, confidence: 0.9)
        ]
    )

    let data = try JSONEncoder().encode(response)
    let json = String(decoding: data, as: UTF8.self)

    #expect(json.contains("fluid_audio_coreml"))
    #expect(json.contains("Hello"))
}
```

- [ ] **Step 5: Resolve Swift dependencies and pin Package.resolved**

Run:

```bash
rtk swift package --package-path src-tauri/native/fluidaudio-parakeet resolve
```

Expected: `src-tauri/native/fluidaudio-parakeet/Package.resolved` is created and pins FluidAudio to `3c6e79f1d74411cae1f3daf50260dd19a585dc2d`.

- [ ] **Step 6: Run Swift tests**

Run:

```bash
rtk swift test --package-path src-tauri/native/fluidaudio-parakeet
```

Expected: PASS. If the command fails because a FluidAudio symbol name differs at the pinned revision, update only `Protocol.swift` to the pinned API while preserving the request/response JSON shapes in this plan.

- [ ] **Step 7: Commit**

Run:

```bash
rtk git add src-tauri/native/fluidaudio-parakeet
rtk git commit -m "feat: add native FluidAudio transcription helper"
```

Expected: commit succeeds.

---

## Task 4: Add Rust FluidAudio Adapter And Helper Parser

**Files:**
- Create: `src-tauri/src/transcription/fluidaudio.rs`
- Modify: `src-tauri/src/transcription/mod.rs`
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing helper parser test**

Add to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn fluid_audio_helper_response_converts_words_to_runtime_tokens() {
    let json = serde_json::json!({
        "schemaVersion": 1,
        "runtimeId": "fluid_audio_coreml",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "text": "Hello world",
        "durationSeconds": 1.2,
        "processingSeconds": 0.4,
        "words": [
            { "text": "Hello", "startSeconds": 0.1, "endSeconds": 0.4, "confidence": 0.9 },
            { "text": "world", "startSeconds": 0.5, "endSeconds": 0.9, "confidence": null }
        ]
    });

    let output = video_creater_lib::transcription::fluidaudio::runtime_output_from_helper_json(
        "nvidia/parakeet-tdt-0.6b-v3",
        &serde_json::to_vec(&json).expect("json bytes"),
    )
    .expect("runtime output");

    assert_eq!(output.runtime_id, "fluid_audio_coreml");
    assert_eq!(output.model_id, "nvidia/parakeet-tdt-0.6b-v3");
    assert_eq!(output.tokens.len(), 2);
    assert_eq!(output.tokens[0].token, "Hello");
    assert_eq!(output.tokens[0].start, 0.1);
    assert_eq!(output.tokens[0].end, 0.4);
}

#[test]
fn fluid_audio_helper_response_rejects_empty_words() {
    let json = serde_json::json!({
        "schemaVersion": 1,
        "runtimeId": "fluid_audio_coreml",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "text": "",
        "durationSeconds": 0.0,
        "processingSeconds": 0.1,
        "words": []
    });

    let error = video_creater_lib::transcription::fluidaudio::runtime_output_from_helper_json(
        "nvidia/parakeet-tdt-0.6b-v3",
        &serde_json::to_vec(&json).expect("json bytes"),
    )
    .expect_err("empty words should fail");

    assert!(error.to_string().contains("no usable words"));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models fluid_audio_helper_response_ -- --nocapture
```

Expected: FAIL because `transcription::fluidaudio` does not exist.

- [ ] **Step 3: Implement helper protocol parser**

Create `src-tauri/src/transcription/fluidaudio.rs`:

```rust
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::model::{TranscriptionModelArtifactFormat, FLUID_AUDIO_COREML_RUNTIME_ID};
use super::runtime::{
    InstalledModel, ModelModality, ModelRuntime, RuntimeCapability, TranscriptToken,
    TranscriptionRuntimeEngine, TranscriptionRuntimeError, TranscriptionRuntimeJob,
    TranscriptionRuntimeOutput,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FluidAudioHelperRequest<'a> {
    schema_version: u32,
    model_id: &'a str,
    model_path: &'a str,
    media_path: &'a str,
    language_mode: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FluidAudioHelperResponse {
    schema_version: u32,
    runtime_id: String,
    model_id: String,
    text: String,
    duration_seconds: f64,
    processing_seconds: f64,
    words: Vec<FluidAudioHelperWord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FluidAudioHelperWord {
    text: String,
    start_seconds: f64,
    end_seconds: f64,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FluidAudioHelperError {
    error: String,
}

#[derive(Debug, Clone)]
pub struct FluidAudioCoreMlRuntime {
    helper_path: PathBuf,
}

impl FluidAudioCoreMlRuntime {
    pub fn new(helper_path: PathBuf) -> Self {
        Self { helper_path }
    }
}

pub fn runtime_output_from_helper_json(
    expected_model_id: &str,
    stdout: &[u8],
) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
    if let Ok(error) = serde_json::from_slice::<FluidAudioHelperError>(stdout) {
        return Err(TranscriptionRuntimeError::Inference(error.error));
    }

    let response: FluidAudioHelperResponse = serde_json::from_slice(stdout).map_err(|error| {
        TranscriptionRuntimeError::Inference(format!("failed to parse FluidAudio helper response: {error}"))
    })?;
    if response.schema_version != 1 {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "unsupported FluidAudio helper response schemaVersion {}",
            response.schema_version
        )));
    }
    if response.runtime_id != FLUID_AUDIO_COREML_RUNTIME_ID {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "FluidAudio helper runtimeId mismatch: expected {}, found {}",
            FLUID_AUDIO_COREML_RUNTIME_ID, response.runtime_id
        )));
    }
    if response.model_id != expected_model_id {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "FluidAudio helper modelId mismatch: expected {}, found {}",
            expected_model_id, response.model_id
        )));
    }

    let tokens = response
        .words
        .into_iter()
        .filter_map(|word| {
            let text = word.text.trim().to_string();
            if text.is_empty() || word.end_seconds < word.start_seconds {
                return None;
            }
            Some(TranscriptToken {
                token: text,
                start: word.start_seconds,
                end: word.end_seconds,
                confidence: word.confidence,
            })
        })
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return Err(TranscriptionRuntimeError::Inference(
            "FluidAudio helper returned no usable words".to_string(),
        ));
    }

    Ok(TranscriptionRuntimeOutput {
        runtime_id: response.runtime_id,
        model_id: response.model_id,
        tokens,
    })
}
```

- [ ] **Step 4: Implement runtime support and process invocation**

In `src-tauri/src/transcription/fluidaudio.rs`, add:

```rust
impl ModelRuntime for FluidAudioCoreMlRuntime {
    fn runtime_id(&self) -> &'static str {
        FLUID_AUDIO_COREML_RUNTIME_ID
    }

    fn modality(&self) -> ModelModality {
        ModelModality::Transcription
    }

    fn supports_installed_model(&self, model: &InstalledModel) -> bool {
        model.modality == ModelModality::Transcription
            && model.runtime_family == FLUID_AUDIO_COREML_RUNTIME_ID
            && model.artifact_format == TranscriptionModelArtifactFormat::CoreMlBundle
    }

    fn probe_installed_model(&self, model: &InstalledModel) -> RuntimeCapability {
        if !self.supports_installed_model(model) {
            return RuntimeCapability::Unavailable;
        }
        #[cfg(target_os = "macos")]
        {
            if self.helper_path.is_file() {
                RuntimeCapability::Ready
            } else {
                RuntimeCapability::Unavailable
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            RuntimeCapability::UnsupportedPlatform
        }
    }
}

impl TranscriptionRuntimeEngine for FluidAudioCoreMlRuntime {
    fn transcribe_installed(
        &self,
        job: &TranscriptionRuntimeJob,
        model: &InstalledModel,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = job;
            let _ = model;
            Err(TranscriptionRuntimeError::UnsupportedPlatform)
        }
        #[cfg(target_os = "macos")]
        {
            let request = FluidAudioHelperRequest {
                schema_version: 1,
                model_id: &model.model_id,
                model_path: &model.model_dir.display().to_string(),
                media_path: &job.source_path,
                language_mode: &job.language_mode,
            };
            let input = serde_json::to_vec(&request).map_err(|error| {
                TranscriptionRuntimeError::Inference(format!("failed to serialize FluidAudio helper request: {error}"))
            })?;
            let mut child = Command::new(&self.helper_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| {
                    TranscriptionRuntimeError::Inference(format!(
                        "failed to start FluidAudio helper {}: {error}",
                        self.helper_path.display()
                    ))
                })?;
            {
                use std::io::Write;
                let stdin = child.stdin.as_mut().ok_or_else(|| {
                    TranscriptionRuntimeError::Inference("failed to open FluidAudio helper stdin".to_string())
                })?;
                stdin.write_all(&input).map_err(|error| {
                    TranscriptionRuntimeError::Inference(format!("failed to write FluidAudio helper request: {error}"))
                })?;
            }
            let output = child.wait_with_output().map_err(|error| {
                TranscriptionRuntimeError::Inference(format!("failed to read FluidAudio helper response: {error}"))
            })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.trim().is_empty() {
                    return Err(TranscriptionRuntimeError::Inference(format!(
                        "FluidAudio helper failed: {}",
                        stderr.trim()
                    )));
                }
            }
            runtime_output_from_helper_json(&model.model_id, &output.stdout)
        }
    }
}
```

- [ ] **Step 5: Export module and run parser tests**

In `src-tauri/src/transcription/mod.rs`, add:

```rust
pub mod fluidaudio;
```

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models fluid_audio_helper_response_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/transcription/fluidaudio.rs src-tauri/src/transcription/mod.rs src-tauri/src/transcription/runtime.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add Rust FluidAudio runtime adapter"
```

Expected: commit succeeds.

---

## Task 5: Route Temporal RunTranscription Through The Runtime Registry

**Files:**
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Write failing registry-dispatch Temporal test**

Add to `src-tauri/tests/temporal_workflows.rs` near the existing run transcription tests:

```rust
#[test]
fn run_transcription_activity_uses_selected_runtime_registry() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");
    let registry = video_creater_lib::transcription::runtime::TranscriptionRuntimeRegistry::new(
        vec![Box::new(video_creater_lib::transcription::runtime::FixtureTranscriptionRuntime {
            runtime_id: "fluid_audio_coreml",
            capability: video_creater_lib::transcription::runtime::RuntimeCapability::Ready,
            supported: true,
            tokens: vec![TranscriptToken {
                token: "Hello".to_string(),
                start: 0.1,
                end: 0.4,
                confidence: Some(0.95),
            }],
        })],
    );

    let output = video_creater_lib::workflows::temporal_transcribe_run_activity_value_with_registry(
        json!({
            "status": "ready",
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.wav",
            "artifactPath": root.path().join("transcripts/transcribe-1-transcript.json").display().to_string(),
            "mediaKind": "audio",
            "mediaRelativePath": "media/source.wav",
            "mediaDurationSeconds": 12.0,
            "mediaWidth": null,
            "mediaHeight": null,
            "mediaFps": null,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": "fluid_audio_coreml"
        }),
        &registry,
        &artifact_dir,
    )
    .expect("run transcription");

    assert_eq!(output["runtimeId"], "fluid_audio_coreml");
    let artifact_path = output["artifactPath"].as_str().expect("artifact path");
    let artifact: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(artifact_path).expect("artifact json"))
            .expect("parse artifact");
    assert_eq!(artifact["engine"], "fluid_audio_coreml");
    assert_eq!(artifact["tokens"][0]["token"], "Hello");
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows run_transcription_activity_uses_selected_runtime_registry -- --nocapture
```

Expected: FAIL because `temporal_transcribe_run_activity_value_with_registry` does not exist.

- [ ] **Step 3: Add installed model construction helper**

In `src-tauri/src/transcription/store.rs`, add:

```rust
pub fn installed_model_for_status(
    &self,
    status: &TranscriptionModelStatus,
) -> Result<crate::transcription::runtime::InstalledModel, ModelStoreError> {
    let entry = self.catalog_entry(&status.model_id)?;
    Ok(crate::transcription::runtime::InstalledModel {
        model_id: entry.id.to_string(),
        model_dir: self.model_dir_for_entry(&entry),
        artifact_format: entry.artifact_format,
        modality: crate::transcription::runtime::ModelModality::Transcription,
        runtime_family: entry.runtime_family.to_string(),
    })
}
```

- [ ] **Step 4: Add registry-based workflow function**

In `src-tauri/src/workflows/mod.rs`, add a new function beside `temporal_transcribe_run_activity_value_with_backend`:

```rust
pub fn temporal_transcribe_run_activity_value_with_registry(
    input: Value,
    registry: &TranscriptionRuntimeRegistry,
    artifact_dir: &Path,
) -> Result<Value, TemporalWorkflowInputError> {
    let probe: TemporalTranscribeProbeOutput = serde_json::from_value(input)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    validate_transcribe_run_input(&probe)?;

    let entry = transcription_model_catalog_entry(&probe.model_id).ok_or_else(|| {
        TemporalWorkflowInputError::Transcription(format!(
            "transcription model {} is not in the catalog",
            probe.model_id
        ))
    })?;
    let installed = InstalledModel {
        model_id: entry.id.to_string(),
        model_dir: std::path::PathBuf::from(&probe.model_path),
        artifact_format: entry.artifact_format,
        modality: ModelModality::Transcription,
        runtime_family: entry.runtime_family.to_string(),
    };
    let runtime = registry
        .select(&probe.runtime_id, &installed)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let job = TranscriptionRuntimeJob {
        media_id: probe.media_id.clone(),
        source_path: probe.source_path.clone(),
        model_path: probe.model_path.clone(),
        language_mode: probe.language_mode.clone(),
    };
    let output = runtime
        .transcribe_installed(&job, &installed)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;

    write_transcription_runtime_artifact(probe, job, output, artifact_dir)
}
```

Extract the artifact-writing logic from `temporal_transcribe_run_activity_value_with_backend` into:

```rust
fn write_transcription_runtime_artifact(
    probe: TemporalTranscribeProbeOutput,
    job: TranscriptionRuntimeJob,
    output: TranscriptionRuntimeOutput,
    artifact_dir: &Path,
) -> Result<Value, TemporalWorkflowInputError> {
    if output.model_id != probe.model_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "modelId".to_string(),
        ));
    }
    if output.runtime_id.trim().is_empty() {
        return Err(TemporalWorkflowInputError::BlankField(
            "runtimeId".to_string(),
        ));
    }
    if output.runtime_id != probe.runtime_id {
        return Err(TemporalWorkflowInputError::MismatchedInputField(
            "runtimeId".to_string(),
        ));
    }
    let token_count = output.tokens.len();
    let artifact_path = artifact_dir.join(format!("{}-transcript.json", probe.job_id));
    let artifact = json!({
        "schemaVersion": 1,
        "mediaId": probe.media_id,
        "engine": output.runtime_id,
        "modelId": output.model_id,
        "runtimeId": output.runtime_id,
        "languageMode": probe.language_mode,
        "tokens": output.tokens,
    });
    parse_transcript_artifact(&artifact)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    fs::create_dir_all(artifact_dir)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    let artifact_bytes = serde_json::to_vec_pretty(&artifact)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    fs::write(&artifact_path, artifact_bytes)
        .map_err(|error| TemporalWorkflowInputError::Transcription(error.to_string()))?;
    serde_json::to_value(TemporalTranscribeRunOutput {
        project_id: probe.project_id,
        project_dir: probe.project_dir,
        media_id: job.media_id,
        job_id: probe.job_id,
        language_mode: job.language_mode,
        model_id: artifact["modelId"].as_str().unwrap_or_default().to_string(),
        runtime_id: artifact["runtimeId"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        artifact_path: artifact_path.display().to_string(),
        token_count,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}
```

- [ ] **Step 5: Make default RunTranscription use the default registry**

In `src-tauri/src/transcription/runtime.rs`, add:

```rust
pub fn default_transcription_runtime_registry() -> TranscriptionRuntimeRegistry {
    let helper_path = std::env::var_os("VIDEO_CREATER_FLUID_AUDIO_HELPER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from("src-tauri/native/fluidaudio-parakeet/.build/release/video-creater-fluidaudio-transcribe")
        });
    TranscriptionRuntimeRegistry::new(vec![Box::new(
        crate::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(helper_path),
    )])
}
```

In `temporal_transcribe_run_activity_value`, replace `CoreMlParakeetBackend` with:

```rust
let registry = default_transcription_runtime_registry();
temporal_transcribe_run_activity_value_with_registry(input, &registry, &artifact_dir)
```

- [ ] **Step 6: Update ProbeMedia runtime id**

In `temporal_transcribe_probe_media_activity_value_with_store`, set the runtime id from the catalog/runtime family:

```rust
let runtime_id = entry.runtime_family.to_string();
```

Do not instantiate `CoreMlParakeetRuntime` in `ProbeMedia`.

- [ ] **Step 7: Run Temporal registry tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows run_transcription_activity_uses_selected_runtime_registry -- --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows run_transcription_activity_writes_runtime_artifact -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
rtk git add src-tauri/src/workflows/mod.rs src-tauri/src/transcription/runtime.rs src-tauri/src/transcription/store.rs src-tauri/tests/temporal_workflows.rs
rtk git commit -m "feat: route transcription workflow through runtime registry"
```

Expected: commit succeeds.

---

## Task 6: Add Real Env-Gated E2E Transcription Proof

**Files:**
- Create: `src-tauri/tests/transcription_e2e.rs`
- Modify: `package.json`
- Modify: `docs/superpowers/specs/2026-06-29-native-model-runtime-e2e-transcription-design.md`

- [ ] **Step 1: Add E2E test target**

Create `src-tauri/tests/transcription_e2e.rs`:

```rust
use serde_json::json;
use std::fs;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{MediaAsset, MediaKind};
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::transcription::store::TranscriptionModelStore;
use video_creater_lib::workflows::{
    temporal_transcribe_probe_media_activity_value_with_store,
    temporal_transcribe_run_activity_value,
    temporal_transcribe_store_activity_value,
};

#[test]
fn temporal_transcription_runs_real_fluidaudio_coreml_e2e_when_model_fixture_is_configured() {
    let Some(model_root) = std::env::var_os("VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT") else {
        eprintln!("skipping real transcription E2E: VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT is not set");
        return;
    };
    let Some(wav_fixture) = std::env::var_os("VIDEO_CREATER_TEST_TRANSCRIPTION_WAV") else {
        eprintln!("skipping real transcription E2E: VIDEO_CREATER_TEST_TRANSCRIPTION_WAV is not set");
        return;
    };
    let Some(helper) = std::env::var_os("VIDEO_CREATER_FLUID_AUDIO_HELPER") else {
        eprintln!("skipping real transcription E2E: VIDEO_CREATER_FLUID_AUDIO_HELPER is not set");
        return;
    };

    assert!(std::path::Path::new(&helper).is_file(), "helper binary must exist");
    let project_dir = tempfile::tempdir().expect("project dir");
    let model_store_root = tempfile::tempdir().expect("model store");
    let media_dir = project_dir.path().join("media");
    fs::create_dir_all(&media_dir).expect("media dir");
    let media_path = media_dir.join("speech.wav");
    fs::copy(&wav_fixture, &media_path).expect("copy wav fixture");

    let mut project = sample_project();
    project.id = "project-e2e".to_string();
    project.media.clear();
    project.media.push(MediaAsset {
        id: "media-speech".to_string(),
        relative_path: "media/speech.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 3.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.transcripts.clear();
    save_split_project(project_dir.path(), &project).expect("save project");

    let store = TranscriptionModelStore::new(model_store_root.path().to_path_buf());
    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", std::path::Path::new(&model_root))
        .expect("import real model");
    assert_eq!(status.install_status, video_creater_lib::transcription::model::ModelInstallStatus::Ready);

    let probe = temporal_transcribe_probe_media_activity_value_with_store(
        json!({
            "projectId": "project-e2e",
            "projectDir": project_dir.path().display().to_string(),
            "mediaId": "media-speech",
            "jobId": "transcribe-real-1",
            "languageMode": "en"
        }),
        &store,
    )
    .expect("probe");
    assert_eq!(probe["runtimeId"], "fluid_audio_coreml");

    let run = temporal_transcribe_run_activity_value(probe).expect("run transcription");
    assert!(run["tokenCount"].as_u64().expect("token count") > 0);

    let stored = temporal_transcribe_store_activity_value(run).expect("store transcript");
    assert_eq!(stored["storedTranscriptId"], "transcript-media-speech");

    let reloaded = load_split_project(project_dir.path()).expect("reload project");
    let transcript = reloaded
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == "media-speech")
        .expect("stored transcript");
    assert!(!transcript.words.is_empty());
    assert!(transcript
        .words
        .windows(2)
        .all(|window| window[1].start_seconds >= window[0].start_seconds));
    assert!(transcript
        .raw_artifact_path
        .as_deref()
        .unwrap_or_default()
        .contains("workflow-artifacts/transcribe-real-1"));
}
```

- [ ] **Step 2: Run E2E test in skip mode**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e -- --nocapture
```

Expected: PASS by printing a skip message when the three env vars are not set.

- [ ] **Step 3: Add helper build script**

In `package.json`, add:

```json
"build:fluidaudio-helper": "swift build --package-path src-tauri/native/fluidaudio-parakeet -c release"
```

Run:

```bash
rtk pnpm build:fluidaudio-helper
```

Expected: helper builds at `src-tauri/native/fluidaudio-parakeet/.build/release/video-creater-fluidaudio-transcribe`.

- [ ] **Step 4: Run real E2E with env vars**

Run with real local paths:

```bash
VIDEO_CREATER_FLUID_AUDIO_HELPER="/Users/olhapi/Documents/video-creater/src-tauri/native/fluidaudio-parakeet/.build/release/video-creater-fluidaudio-transcribe" \
VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT="/absolute/path/to/parakeet-source-or-installed-root" \
VIDEO_CREATER_TEST_TRANSCRIPTION_WAV="/absolute/path/to/short-speech.wav" \
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e temporal_transcription_runs_real_fluidaudio_coreml_e2e_when_model_fixture_is_configured -- --nocapture
```

Expected: PASS with non-empty stored transcript words and monotonic timings. The model import path must accept both flat source roots and roots that already contain `parakeet-tdt-0.6b-v3-coreml`, as implemented in Task 2.

- [ ] **Step 5: Update spec with E2E result**

Append a short verification note to `docs/superpowers/specs/2026-06-29-native-model-runtime-e2e-transcription-design.md`:

```markdown
## E2E Verification

Verified on macOS with `VIDEO_CREATER_FLUID_AUDIO_HELPER`, `VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT`, and `VIDEO_CREATER_TEST_TRANSCRIPTION_WAV` set. The workflow ran `ProbeMedia -> RunTranscription -> StoreTranscript`, produced a raw artifact under `workflow-artifacts/`, and stored non-empty transcript words with monotonic timings in the split project.
```

- [ ] **Step 6: Run full focused verification**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models -- --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows default_coreml_run_activity run_transcription_activity_uses_selected_runtime_registry -- --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e -- --nocapture
rtk swift test --package-path src-tauri/native/fluidaudio-parakeet
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater
```

Expected: all pass. The E2E test may skip only when env vars are unset; before declaring E2E complete, rerun Step 4 with env vars set and record that result.

- [ ] **Step 7: Commit**

Run:

```bash
rtk git add src-tauri/tests/transcription_e2e.rs package.json docs/superpowers/specs/2026-06-29-native-model-runtime-e2e-transcription-design.md
rtk git commit -m "test: verify native transcription workflow end to end"
```

Expected: commit succeeds.

---

## Final Verification

After all tasks are committed, run:

```bash
rtk git status --short --branch
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models -- --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e -- --nocapture
rtk swift test --package-path src-tauri/native/fluidaudio-parakeet
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater
rtk pnpm test src/lib/transcription-models.test.ts
```

Then run the real E2E command from Task 6 Step 4 with the three env vars set. The work is not complete unless that env-gated real E2E run passes with a real model and WAV fixture.
