# Model Loading And Management Design

## Goal

Close the remaining model-environment gap for real native transcription E2E: the app should know where a compatible Core ML model can be acquired, install it into the global model store, verify it, and hand only a local installed path to the runtime.

The first concrete target is FluidAudio's Core ML Parakeet v3 assets on macOS. The acquisition interfaces must stay model- and modality-neutral enough to support future transcription, image generation, and video generation models. Runtime interfaces must not bake in model names, provider names, or bundle file names.

## Research Summary

FluidAudio publishes a Core ML Parakeet v3 model repository on Hugging Face:

- Source repo: [FluidInference/parakeet-tdt-0.6b-v3-coreml](https://huggingface.co/FluidInference/parakeet-tdt-0.6b-v3-coreml)
- Observed source commit: `aed02740059203c4a87495924f685de3722ae9ce`
- Public/gated status from the Hugging Face model API: public, not gated
- Reported license metadata from the Hugging Face model API: `cc-by-4.0`; the model card also references FluidAudio repository licensing, so redistribution or mirroring still needs a separate license review.
- Model card base model: [nvidia/parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)

The NVIDIA base repo contains the original `.nemo` and `safetensors` artifacts, not the compiled `.mlmodelc` layout the current native runtime needs. The app should treat the NVIDIA repo as model lineage metadata only for this slice, not as the active download source.

FluidAudio's [Manual Model Loading guide](https://github.com/FluidInference/FluidAudio/blob/main/Documentation/ASR/ManualModelLoading.md) documents that apps can stage Hugging Face Core ML bundles locally and then call FluidAudio loading APIs without a runtime download. That matches Video Creater's intended boundary: the app model store owns acquisition; `RunTranscription` stays local and offline.

## Current Gap

The current store already has the right foundation:

- one global model root under the app data directory;
- catalog entries with supported runtimes and required files;
- import, verify, active model, remove, and status operations;
- a runtime-local model directory passed into the FluidAudio helper;
- Temporal calling a generic runtime registry instead of branching on a model name.

The gap is acquisition. `download()` explicitly rejects Core ML catalog entries because no remote source is configured. Real E2E also depends on a local media fixture, but that should be solved separately by generating or providing test audio; model acquisition should not depend on per-project media.

## Design Principles

- Model acquisition is global, not project-local.
- Runtime execution is local-only. A runtime receives an installed model path and never downloads.
- Provider details live in catalog/source metadata and provider implementations, not in Temporal or runtime traits.
- Install is staged, validated, and promoted atomically.
- Catalog source revisions are immutable when possible. Use the Hugging Face commit SHA, not `main`, for default downloads.
- The first source can be Hugging Face. Hosting our own mirror is optional fallback infrastructure, not a prerequisite.
- Interfaces describe capabilities and artifact formats, not specific model names.

## Catalog And Source Model

Extend model catalog data with source metadata:

```rust
pub enum ModelModality {
    Transcription,
    ImageGeneration,
    VideoGeneration,
}

pub enum ModelArtifactFormat {
    CoreMlBundle,
    // Future values: MlxBundle, OnnxBundle, DiffusionBundle, VideoGenerationBundle.
}

pub struct ModelCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub modality: ModelModality,
    pub family: String,
    pub artifact_format: ModelArtifactFormat,
    pub approximate_size_bytes: u64,
    pub supported_runtimes: Vec<String>,
    pub required_files: Vec<ModelRequiredFile>,
    pub artifact_sources: Vec<ModelArtifactSource>,
}

pub enum ModelArtifactSource {
    HuggingFaceHub(HuggingFaceArtifactSource),
    LocalImport,
    HttpMirror(HttpMirrorArtifactSource),
}

pub struct HuggingFaceArtifactSource {
    pub repo_id: String,
    pub revision: String,
    pub path_prefix: Option<String>,
    pub include_files: Vec<String>,
    pub license: Option<String>,
}
```

For the current Parakeet catalog entry:

- `id`: remains the stable app model id, currently `nvidia/parakeet-tdt-0.6b-v3`;
- `artifact_sources[0]`: `HuggingFaceHub { repo_id: "FluidInference/parakeet-tdt-0.6b-v3-coreml", revision: "aed02740059203c4a87495924f685de3722ae9ce", ... }`;
- `supported_runtimes`: includes `fluid_audio_coreml`;
- `required_files`: records the minimum runtime-local files expected after install.

The source repo contains multiple related bundles, including `JointDecision.mlmodelc`, `JointDecisionv2.mlmodelc`, `JointDecisionv3.mlmodelc`, both `parakeet_vocab.json` and `parakeet_v3_vocab.json`, and additional `.mlpackage` artifacts. The catalog must therefore be explicit about the file set it installs. Runtime code should not scan the repo and infer a model variant.

## Provider Interface

Add a replaceable acquisition provider layer:

```rust
pub trait ModelArtifactProvider {
    fn provider_id(&self) -> &'static str;

    fn supports(&self, source: &ModelArtifactSource) -> bool;

    fn inspect_remote(
        &self,
        source: &ModelArtifactSource,
    ) -> Result<RemoteModelArtifact, ModelAcquisitionError>;

    fn download(
        &self,
        request: ModelDownloadRequest,
        progress: &dyn ModelDownloadProgress,
    ) -> Result<DownloadedModelArtifact, ModelAcquisitionError>;
}
```

The first provider is `HuggingFaceHubProvider`. It should:

1. Query the Hugging Face model API for repo metadata and file siblings.
2. Verify that the configured revision exists and that required files are present.
3. Download only configured files, not every object in the repo.
4. Use Hugging Face `/resolve/{revision}/{path}` URLs for file content.
5. Preserve source metadata in the installed manifest.

The provider interface allows a future `HttpMirrorProvider` without changing store or runtime consumers.

## Install Flow

The global model store owns install state:

1. User or workflow asks to install a model by generic `model_id`.
2. Store resolves the catalog entry and picks the first supported source unless a source is explicitly selected.
3. Store creates a staging directory under the global model root.
4. Provider downloads files into staging using `.part` files.
5. Store validates required files, nonzero sizes, optional checksums, source revision, and runtime compatibility.
6. Store writes `video-creater-model-manifest.json` into staging.
7. Store writes or refreshes any runtime inspection summary needed by the selected runtime.
8. Store atomically promotes staging into the model's global directory.
9. Store reports the model `Ready`.

The installed layout remains deterministic:

```text
~/Library/Application Support/com.olhapi.video-creater/models/
  nvidia__parakeet-tdt-0.6b-v3/
    video-creater-model-manifest.json
    video-creater-coreml-inspection.json
    fluid-audio-coreml/
      Preprocessor.mlmodelc/
      Encoder.mlmodelc/
      Decoder.mlmodelc/
      JointDecisionv3.mlmodelc/
      parakeet_v3_vocab.json
      config.json
```

The runtime receives the `fluid-audio-coreml` directory from installed model metadata. It does not know the Hugging Face repo id, revision, URL shape, or source file list.

## Manifest Contract

The installed manifest should record enough information to prove what is installed:

```json
{
  "schemaVersion": 1,
  "modelId": "nvidia/parakeet-tdt-0.6b-v3",
  "modality": "transcription",
  "artifactFormat": "core_ml_bundle",
  "source": {
    "provider": "hugging_face_hub",
    "repoId": "FluidInference/parakeet-tdt-0.6b-v3-coreml",
    "revision": "aed02740059203c4a87495924f685de3722ae9ce",
    "license": "cc-by-4.0"
  },
  "runtimeLayouts": {
    "fluid_audio_coreml": {
      "relativePath": "fluid-audio-coreml",
      "requiredFiles": [
        "Preprocessor.mlmodelc/coremldata.bin",
        "Encoder.mlmodelc/coremldata.bin",
        "Decoder.mlmodelc/coremldata.bin",
        "JointDecisionv3.mlmodelc/coremldata.bin",
        "parakeet_v3_vocab.json",
        "config.json"
      ]
    }
  },
  "installedFiles": [],
  "installedAt": "2026-06-29T00:00:00Z",
  "verifiedAt": "2026-06-29T00:00:00Z"
}
```

`installedFiles` should include relative path, size, and sha256 when available. Checksums are preferred. If upstream does not provide stable checksums through the API, Video Creater computes sha256 after download and stores it for future local verification.

## Hosting Decision

Do not host the model for the initial implementation.

Use upstream Hugging Face as the primary source because it already hosts the compiled FluidAudio/Core ML artifact in the expected repository shape. Add an optional mirror source later only if one of these becomes true:

- upstream availability is not reliable enough;
- release builds require a signed or pinned internal manifest;
- download bandwidth or geography requires a CDN;
- product requirements need a source controlled by Video Creater;
- license review confirms redistribution terms and attribution requirements for mirroring.

The catalog shape should support mirrors from day one, but no mirror infrastructure is required for the first working E2E.

## UI And UX

The existing Models UI can stay compact. It should show source-backed install status:

- model display name;
- modality and runtime support;
- installed/missing/downloading/failed state;
- source label, for example `Hugging Face`;
- approximate download size;
- license label or link when available;
- install, cancel, verify, remove, and import actions.

The UI should not expose provider mechanics unless needed for failure recovery. User-facing errors should say what action is needed: network unavailable, source file missing, install cancelled, verification failed, platform unsupported, or model installed but runtime unavailable.

## E2E Verification Strategy

Model acquisition tests should be split by risk:

- Unit tests for source selection, URL construction, manifest writing, and required-file validation.
- Provider tests using a fake HTTP server or fixture API payloads.
- A network-gated provider test that verifies the public Hugging Face repo metadata includes the configured revision and required files.
- A macOS-gated real E2E that can install the model into a temp global store, generate or use a small local speech fixture, run the FluidAudio helper, and assert non-empty transcript words with timings.

The real E2E should not require `VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT` once acquisition is implemented. A separate override may remain useful for offline development, but the default path should prove that the app can bootstrap its own global model store.

## Error Handling

Failures should be typed and specific:

- `SourceUnavailable`: provider cannot reach the source.
- `SourceRevisionMissing`: configured immutable revision no longer resolves.
- `SourceFileMissing`: configured required file is absent from source metadata.
- `DownloadCancelled`: user cancelled an active install.
- `DownloadFailed`: transfer failed or returned an invalid response.
- `VerificationFailed`: installed files are missing, empty, or checksum-invalid.
- `RuntimeUnsupported`: model installed but no local runtime supports it on this platform.
- `LicenseReviewRequired`: source exists but is disabled by policy until accepted.

Temporal should only see installed-model readiness. It should not attempt acquisition during `RunTranscription`.

## Out Of Scope

- Converting `.nemo`, `safetensors`, or `.mlpackage` artifacts into `.mlmodelc`.
- Adding Python, Transformers, or server-side transcription.
- Hosting or mirroring model files in Video Creater infrastructure.
- Adding a second production model.
- Making non-macOS native transcription work in this slice.
- Changing transcript artifact schemas beyond already-supported word timings.

## Acceptance Criteria

- The catalog can describe a model separately from one or more artifact sources.
- Core ML Parakeet has a Hugging Face source pinned to an immutable revision.
- The global model store can install from that source through a provider interface.
- Install writes a manifest with source, runtime layout, and installed files.
- Runtime code receives only an installed local model directory.
- Temporal contracts remain model-agnostic.
- E2E test setup can prove model acquisition plus FluidAudio decode on macOS when network is available.
