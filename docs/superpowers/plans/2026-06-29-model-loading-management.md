# Model Loading Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Core ML model download/install real by loading FluidAudio Parakeet assets from a pinned Hugging Face source into the global model store.

**Architecture:** Extend catalog entries with artifact source metadata, add a small provider layer for Hugging Face-style file acquisition, and route `TranscriptionModelStore::download` through staged provider installs. Runtime and Temporal code continue to receive only installed local model paths and stay free of provider details.

**Tech Stack:** Rust/Tauri, reqwest blocking client, serde, sha2, tempfile, macOS Swift FluidAudio helper for final E2E.

---

## File Structure

- Modify `src-tauri/src/transcription/model.rs`: add generic modality/source structs, pin the FluidAudio Hugging Face Core ML source, and keep existing model id/runtime ids stable.
- Create `src-tauri/src/transcription/acquisition.rs`: implement provider-neutral acquisition types plus the first `HuggingFaceHubProvider`.
- Modify `src-tauri/src/transcription/mod.rs`: export the new acquisition module.
- Modify `src-tauri/src/transcription/store.rs`: replace the Core ML download rejection with provider-backed staged install, manifest source metadata, and sha256 capture.
- Modify `src-tauri/tests/transcription_models.rs`: add focused tests for catalog source metadata, provider URL/file selection, staged install, cancellation, manifest source fields, and runtime-local layout.
- Modify `src-tauri/tests/transcription_e2e.rs`: allow the macOS real E2E to bootstrap a temp global model store through `store.download()` when explicitly requested.
- No frontend files are planned. The existing `download_transcription_model` Tauri command already calls `store.download`.

## Task 1: Add Catalog Source Metadata

**Files:**
- Modify: `src-tauri/src/transcription/model.rs`
- Test: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write the failing catalog source test**

Append this test near the existing catalog tests in `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn parakeet_v3_catalog_entry_declares_pinned_hugging_face_coreml_source() {
    let entry = parakeet_v3_catalog_entry();

    assert_eq!(entry.modality, video_creater_lib::transcription::model::ModelModality::Transcription);
    assert_eq!(entry.artifact_sources.len(), 1);

    let source = &entry.artifact_sources[0];
    assert_eq!(source.provider, video_creater_lib::transcription::model::ModelArtifactProviderKind::HuggingFaceHub);
    assert_eq!(source.repo_id, "FluidInference/parakeet-tdt-0.6b-v3-coreml");
    assert_eq!(source.revision, "aed02740059203c4a87495924f685de3722ae9ce");
    assert_eq!(source.license.as_deref(), Some("cc-by-4.0"));
    assert!(source.include_files.contains(&"Preprocessor.mlmodelc/coremldata.bin".to_string()));
    assert!(source.include_files.contains(&"Encoder.mlmodelc/coremldata.bin".to_string()));
    assert!(source.include_files.contains(&"Decoder.mlmodelc/coremldata.bin".to_string()));
    assert!(source.include_files.contains(&"JointDecisionv3.mlmodelc/coremldata.bin".to_string()));
    assert!(source.include_files.contains(&"parakeet_v3_vocab.json".to_string()));
    assert!(source.include_files.contains(&"config.json".to_string()));
}
```

- [ ] **Step 2: Run the failing catalog source test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models parakeet_v3_catalog_entry_declares_pinned_hugging_face_coreml_source -- --nocapture
```

Expected: FAIL because `ModelModality`, `ModelArtifactProviderKind`, and `artifact_sources` do not exist in `transcription::model`.

- [ ] **Step 3: Add source types and catalog metadata**

In `src-tauri/src/transcription/model.rs`, add these definitions near the existing model enums:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelModality {
    Transcription,
    ImageGeneration,
    VideoGeneration,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactProviderKind {
    HuggingFaceHub,
    LocalImport,
    HttpMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelArtifactSource {
    pub provider: ModelArtifactProviderKind,
    pub repo_id: String,
    pub revision: String,
    pub path_prefix: Option<String>,
    pub include_files: Vec<String>,
    pub license: Option<String>,
}
```

Extend `TranscriptionModelCatalogEntry`:

```rust
pub struct TranscriptionModelCatalogEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub modality: ModelModality,
    pub family: TranscriptionModelFamily,
    pub revision: &'static str,
    pub artifact_format: TranscriptionModelArtifactFormat,
    pub approximate_size_bytes: u64,
    pub supported_runtimes: Vec<TranscriptionRuntimeId>,
    pub required_files: Vec<TranscriptionModelFile>,
    pub artifact_sources: Vec<ModelArtifactSource>,
}
```

Add helper constants/functions:

```rust
pub const FLUID_AUDIO_PARAKEET_V3_COREML_REPO_ID: &str =
    "FluidInference/parakeet-tdt-0.6b-v3-coreml";
pub const FLUID_AUDIO_PARAKEET_V3_COREML_REVISION: &str =
    "aed02740059203c4a87495924f685de3722ae9ce";

fn parakeet_v3_required_files() -> Vec<TranscriptionModelFile> {
    vec![
        required_file("config.json"),
        required_file("parakeet_v3_vocab.json"),
        required_file("Encoder.mlmodelc/coremldata.bin"),
        required_file("Encoder.mlmodelc/metadata.json"),
        required_file("Encoder.mlmodelc/model.mil"),
        required_file("Encoder.mlmodelc/weights/weight.bin"),
        required_file("Decoder.mlmodelc/coremldata.bin"),
        required_file("Decoder.mlmodelc/metadata.json"),
        required_file("Decoder.mlmodelc/model.mil"),
        required_file("Decoder.mlmodelc/weights/weight.bin"),
        required_file("JointDecisionv3.mlmodelc/coremldata.bin"),
        required_file("JointDecisionv3.mlmodelc/metadata.json"),
        required_file("JointDecisionv3.mlmodelc/model.mil"),
        required_file("JointDecisionv3.mlmodelc/weights/weight.bin"),
        required_file("Preprocessor.mlmodelc/coremldata.bin"),
        required_file("Preprocessor.mlmodelc/metadata.json"),
        required_file("Preprocessor.mlmodelc/model.mil"),
        required_file("Preprocessor.mlmodelc/weights/weight.bin"),
    ]
}

fn parakeet_v3_coreml_source(include_files: &[TranscriptionModelFile]) -> ModelArtifactSource {
    ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: FLUID_AUDIO_PARAKEET_V3_COREML_REPO_ID.to_string(),
        revision: FLUID_AUDIO_PARAKEET_V3_COREML_REVISION.to_string(),
        path_prefix: None,
        include_files: include_files.iter().map(|file| file.path.clone()).collect(),
        license: Some("cc-by-4.0".to_string()),
    }
}
```

Update `parakeet_v3_catalog_entry()` so it builds `required_files` once and sets `modality` plus `artifact_sources`:

```rust
pub fn parakeet_v3_catalog_entry() -> TranscriptionModelCatalogEntry {
    let required_files = parakeet_v3_required_files();
    TranscriptionModelCatalogEntry {
        id: "nvidia/parakeet-tdt-0.6b-v3",
        display_name: "Parakeet TDT 0.6B v3",
        provider: "NVIDIA",
        modality: ModelModality::Transcription,
        family: TranscriptionModelFamily::Parakeet,
        revision: FLUID_AUDIO_PARAKEET_V3_COREML_REVISION,
        artifact_format: TranscriptionModelArtifactFormat::CoreMlBundle,
        approximate_size_bytes: 485_000_000,
        supported_runtimes: vec![TranscriptionRuntimeId::FluidAudioCoreMl],
        artifact_sources: vec![parakeet_v3_coreml_source(&required_files)],
        required_files,
    }
}
```

- [ ] **Step 4: Run the catalog source test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models parakeet_v3_catalog_entry_declares_pinned_hugging_face_coreml_source -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run existing model tests for regressions**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit catalog source metadata**

Run:

```bash
rtk git add src-tauri/src/transcription/model.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add model artifact source metadata"
```

Expected: commit succeeds.

## Task 2: Add Acquisition Provider Types

**Files:**
- Create: `src-tauri/src/transcription/acquisition.rs`
- Modify: `src-tauri/src/transcription/mod.rs`
- Test: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing provider URL tests**

Append these tests to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn hugging_face_provider_builds_pinned_resolve_urls() {
    let entry = parakeet_v3_catalog_entry();
    let source = entry.artifact_sources.first().expect("source");

    let url = video_creater_lib::transcription::acquisition::hugging_face_resolve_url(
        source,
        "Encoder.mlmodelc/coremldata.bin",
    );

    assert_eq!(
        url,
        "https://huggingface.co/FluidInference/parakeet-tdt-0.6b-v3-coreml/resolve/aed02740059203c4a87495924f685de3722ae9ce/Encoder.mlmodelc/coremldata.bin"
    );
}

#[test]
fn hugging_face_provider_rejects_missing_required_source_file() {
    let entry = parakeet_v3_catalog_entry();
    let source = entry.artifact_sources.first().expect("source");
    let remote = video_creater_lib::transcription::acquisition::RemoteModelArtifact {
        repo_id: source.repo_id.clone(),
        revision: source.revision.clone(),
        siblings: vec!["config.json".to_string()],
    };

    let result = video_creater_lib::transcription::acquisition::validate_remote_required_files(
        source,
        &remote,
    );

    let error = result.expect_err("missing required files should fail");
    assert!(error.to_string().contains("Encoder.mlmodelc/coremldata.bin"));
}
```

- [ ] **Step 2: Run the failing provider tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models hugging_face_provider_ -- --nocapture
```

Expected: FAIL because the `transcription::acquisition` module does not exist.

- [ ] **Step 3: Implement provider-neutral acquisition module**

Create `src-tauri/src/transcription/acquisition.rs`:

```rust
use super::model::{ModelArtifactProviderKind, ModelArtifactSource};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelAcquisitionError {
    #[error("unsupported artifact source provider {0:?}")]
    UnsupportedProvider(ModelArtifactProviderKind),
    #[error("download cancelled")]
    Cancelled,
    #[error("source repo mismatch: expected {expected}, found {found}")]
    SourceRepoMismatch { expected: String, found: String },
    #[error("source revision mismatch: expected {expected}, found {found}")]
    SourceRevisionMismatch { expected: String, found: String },
    #[error("source file missing: {0}")]
    SourceFileMissing(String),
    #[error("failed to inspect source: {0}")]
    Inspect(String),
    #[error("failed to download source file: {0}")]
    Download(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModelArtifact {
    pub repo_id: String,
    pub revision: String,
    pub siblings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedModelFile {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedModelArtifact {
    pub source: ModelArtifactSource,
    pub files: Vec<DownloadedModelFile>,
}

pub trait ModelDownloadProgress {
    fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError>;
    fn file_completed(&self, downloaded_files: u32) -> Result<(), ModelAcquisitionError>;
}

pub trait ModelArtifactProvider {
    fn provider_id(&self) -> &'static str;
    fn supports(&self, source: &ModelArtifactSource) -> bool;
    fn inspect_remote(
        &self,
        source: &ModelArtifactSource,
    ) -> Result<RemoteModelArtifact, ModelAcquisitionError>;
    fn download(
        &self,
        source: &ModelArtifactSource,
        target_dir: &Path,
        progress: &dyn ModelDownloadProgress,
    ) -> Result<DownloadedModelArtifact, ModelAcquisitionError>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HuggingFaceModelResponse {
    id: String,
    sha: String,
    siblings: Vec<HuggingFaceSibling>,
}

#[derive(Debug, Deserialize)]
struct HuggingFaceSibling {
    rfilename: String,
}

pub struct HuggingFaceHubProvider {
    client: reqwest::blocking::Client,
    api_base_url: String,
    resolve_base_url: String,
}

impl HuggingFaceHubProvider {
    pub fn new(client: reqwest::blocking::Client) -> Self {
        Self::with_base_urls(
            client,
            "https://huggingface.co/api/models",
            "https://huggingface.co",
        )
    }

    pub fn with_base_urls(
        client: reqwest::blocking::Client,
        api_base_url: impl Into<String>,
        resolve_base_url: impl Into<String>,
    ) -> Self {
        Self {
            client,
            api_base_url: api_base_url.into().trim_end_matches('/').to_string(),
            resolve_base_url: resolve_base_url.into().trim_end_matches('/').to_string(),
        }
    }
}

pub fn hugging_face_resolve_url(source: &ModelArtifactSource, file_path: &str) -> String {
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        source.repo_id, source.revision, file_path
    )
}

pub fn validate_remote_required_files(
    source: &ModelArtifactSource,
    remote: &RemoteModelArtifact,
) -> Result<(), ModelAcquisitionError> {
    if remote.repo_id != source.repo_id {
        return Err(ModelAcquisitionError::SourceRepoMismatch {
            expected: source.repo_id.clone(),
            found: remote.repo_id.clone(),
        });
    }
    if remote.revision != source.revision {
        return Err(ModelAcquisitionError::SourceRevisionMismatch {
            expected: source.revision.clone(),
            found: remote.revision.clone(),
        });
    }
    for file in &source.include_files {
        if !remote.siblings.iter().any(|sibling| sibling == file) {
            return Err(ModelAcquisitionError::SourceFileMissing(file.clone()));
        }
    }
    Ok(())
}
```

Add to `src-tauri/src/transcription/mod.rs`:

```rust
pub mod acquisition;
```

- [ ] **Step 4: Run provider type tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models hugging_face_provider_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit provider types**

Run:

```bash
rtk git add src-tauri/src/transcription/acquisition.rs src-tauri/src/transcription/mod.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add model acquisition provider types"
```

Expected: commit succeeds.

## Task 3: Implement Hugging Face Metadata Inspection

**Files:**
- Modify: `src-tauri/src/transcription/acquisition.rs`
- Test: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write a failing provider inspection unit test**

Append this test to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn hugging_face_provider_parses_model_api_response() {
    let json = serde_json::json!({
        "id": "FluidInference/parakeet-tdt-0.6b-v3-coreml",
        "sha": "aed02740059203c4a87495924f685de3722ae9ce",
        "siblings": [
            { "rfilename": "config.json" },
            { "rfilename": "parakeet_v3_vocab.json" },
            { "rfilename": "Encoder.mlmodelc/coremldata.bin" }
        ]
    });

    let remote =
        video_creater_lib::transcription::acquisition::parse_hugging_face_model_response(&json)
            .expect("parse response");

    assert_eq!(remote.repo_id, "FluidInference/parakeet-tdt-0.6b-v3-coreml");
    assert_eq!(remote.revision, "aed02740059203c4a87495924f685de3722ae9ce");
    assert_eq!(
        remote.siblings,
        vec![
            "config.json".to_string(),
            "parakeet_v3_vocab.json".to_string(),
            "Encoder.mlmodelc/coremldata.bin".to_string()
        ]
    );
}
```

- [ ] **Step 2: Run the failing parse test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models hugging_face_provider_parses_model_api_response -- --nocapture
```

Expected: FAIL because `parse_hugging_face_model_response` does not exist.

- [ ] **Step 3: Add response parsing and provider inspection**

In `src-tauri/src/transcription/acquisition.rs`, add:

```rust
pub fn parse_hugging_face_model_response(
    value: &serde_json::Value,
) -> Result<RemoteModelArtifact, ModelAcquisitionError> {
    let response: HuggingFaceModelResponse = serde_json::from_value(value.clone())
        .map_err(|error| ModelAcquisitionError::Inspect(error.to_string()))?;
    Ok(RemoteModelArtifact {
        repo_id: response.id,
        revision: response.sha,
        siblings: response
            .siblings
            .into_iter()
            .map(|sibling| sibling.rfilename)
            .collect(),
    })
}

impl ModelArtifactProvider for HuggingFaceHubProvider {
    fn provider_id(&self) -> &'static str {
        "hugging_face_hub"
    }

    fn supports(&self, source: &ModelArtifactSource) -> bool {
        source.provider == ModelArtifactProviderKind::HuggingFaceHub
    }

    fn inspect_remote(
        &self,
        source: &ModelArtifactSource,
    ) -> Result<RemoteModelArtifact, ModelAcquisitionError> {
        if !self.supports(source) {
            return Err(ModelAcquisitionError::UnsupportedProvider(source.provider));
        }
        let url = format!("{}/{}", self.api_base_url, source.repo_id);
        let value = self
            .client
            .get(&url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| ModelAcquisitionError::Inspect(error.to_string()))?
            .json::<serde_json::Value>()
            .map_err(|error| ModelAcquisitionError::Inspect(error.to_string()))?;
        let remote = parse_hugging_face_model_response(&value)?;
        validate_remote_required_files(source, &remote)?;
        Ok(remote)
    }

    fn download(
        &self,
        _source: &ModelArtifactSource,
        _target_dir: &Path,
        _progress: &dyn ModelDownloadProgress,
    ) -> Result<DownloadedModelArtifact, ModelAcquisitionError> {
        Err(ModelAcquisitionError::Download(
            "download is not implemented yet".to_string(),
        ))
    }
}
```

- [ ] **Step 4: Run provider inspection tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models hugging_face_provider_ -- --nocapture
```

Expected: PASS for URL, validation, and parse tests.

- [ ] **Step 5: Add a network-gated metadata test**

Append this test to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn hugging_face_provider_can_inspect_real_parakeet_source_when_network_enabled() {
    if std::env::var("VIDEO_CREATER_TEST_HF_MODEL_METADATA").ok().as_deref() != Some("1") {
        eprintln!("skipping Hugging Face metadata test: set VIDEO_CREATER_TEST_HF_MODEL_METADATA=1");
        return;
    }

    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelArtifactProvider,
    };

    let entry = parakeet_v3_catalog_entry();
    let source = entry.artifact_sources.first().expect("source");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("client");
    let provider = HuggingFaceHubProvider::new(client);

    let remote = provider.inspect_remote(source).expect("inspect real HF source");

    assert_eq!(remote.repo_id, source.repo_id);
    assert_eq!(remote.revision, source.revision);
    assert!(remote.siblings.contains(&"config.json".to_string()));
    assert!(remote.siblings.contains(&"JointDecisionv3.mlmodelc/coremldata.bin".to_string()));
}
```

- [ ] **Step 6: Run network-gated metadata test in skip mode**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models hugging_face_provider_can_inspect_real_parakeet_source_when_network_enabled -- --nocapture
```

Expected: PASS with skip message.

- [ ] **Step 7: Commit metadata inspection**

Run:

```bash
rtk git add src-tauri/src/transcription/acquisition.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: inspect model artifact sources"
```

Expected: commit succeeds.

## Task 4: Implement Provider Download To Staging

**Files:**
- Modify: `src-tauri/src/transcription/acquisition.rs`
- Test: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing local-file download test**

Append this test to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn hugging_face_provider_downloads_included_files_and_records_sha256() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(
            &self,
        ) -> Result<bool, video_creater_lib::transcription::acquisition::ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(
            &self,
            _downloaded_files: u32,
        ) -> Result<(), video_creater_lib::transcription::acquisition::ModelAcquisitionError> {
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let encoder = source_dir.path().join("resolve").join("Encoder.mlmodelc").join("coremldata.bin");
    std::fs::create_dir_all(encoder.parent().expect("parent")).expect("mkdir");
    std::fs::write(&encoder, b"encoder bytes").expect("write encoder");
    let config = source_dir.path().join("resolve").join("config.json");
    std::fs::write(&config, b"{\"model\":\"fixture\"}").expect("write config");

    let mut entry = parakeet_v3_catalog_entry();
    let mut source = entry.artifact_sources.remove(0);
    source.include_files = vec![
        "Encoder.mlmodelc/coremldata.bin".to_string(),
        "config.json".to_string(),
    ];

    let client = reqwest::blocking::Client::builder().build().expect("client");
    let provider = HuggingFaceHubProvider::with_base_urls(
        client,
        "http://127.0.0.1/unused",
        source_dir.path().join("resolve").to_string_lossy().to_string(),
    );

    let artifact = provider
        .download(&source, target_dir.path(), &NoCancel)
        .expect("download local fixture files");

    assert_eq!(artifact.files.len(), 2);
    assert_eq!(
        std::fs::read(target_dir.path().join("Encoder.mlmodelc/coremldata.bin")).expect("read"),
        b"encoder bytes"
    );
    let encoder_file = artifact
        .files
        .iter()
        .find(|file| file.relative_path == "Encoder.mlmodelc/coremldata.bin")
        .expect("encoder file");
    assert_eq!(encoder_file.size_bytes, "encoder bytes".len() as u64);
    assert_eq!(
        encoder_file.sha256,
        "f0cf98ee8f826096ea85c67028b74638b2967d815b670df19414e6e10f03b87e"
    );
}
```

- [ ] **Step 2: Run the failing download test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models hugging_face_provider_downloads_included_files_and_records_sha256 -- --nocapture
```

Expected: FAIL because provider download still returns a fixed error and file URLs are not handled.

- [ ] **Step 3: Implement file download, part files, and sha256 capture**

In `src-tauri/src/transcription/acquisition.rs`, update imports:

```rust
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
```

Add helper functions:

```rust
fn source_file_url(base_url: &str, source: &ModelArtifactSource, file_path: &str) -> String {
    if base_url.starts_with("http://") || base_url.starts_with("https://") {
        return format!(
            "{}/{}/resolve/{}/{}",
            base_url.trim_end_matches('/'),
            source.repo_id,
            source.revision,
            file_path
        );
    }
    Path::new(base_url)
        .join(file_path)
        .to_string_lossy()
        .to_string()
}

fn copy_local_file_to_part(source_path: &Path, part_path: &Path) -> Result<(), ModelAcquisitionError> {
    fs::copy(source_path, part_path)
        .map(|_| ())
        .map_err(|error| ModelAcquisitionError::Download(error.to_string()))
}

fn sha256_file(path: &Path) -> Result<String, ModelAcquisitionError> {
    let mut file = fs::File::open(path)
        .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
```

Replace the fixed-error `download` method with:

```rust
    fn download(
        &self,
        source: &ModelArtifactSource,
        target_dir: &Path,
        progress: &dyn ModelDownloadProgress,
    ) -> Result<DownloadedModelArtifact, ModelAcquisitionError> {
        if !self.supports(source) {
            return Err(ModelAcquisitionError::UnsupportedProvider(source.provider));
        }

        let mut files = Vec::new();
        for (index, relative_path) in source.include_files.iter().enumerate() {
            if progress.is_cancelled()? {
                return Err(ModelAcquisitionError::Cancelled);
            }

            let target_path = target_dir.join(relative_path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
            }
            let part_path = target_path.with_extension("part");
            let url = source_file_url(&self.resolve_base_url, source, relative_path);
            if url.starts_with("http://") || url.starts_with("https://") {
                let mut response = self
                    .client
                    .get(&url)
                    .send()
                    .and_then(|response| response.error_for_status())
                    .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
                let mut part_file = fs::File::create(&part_path)
                    .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    if progress.is_cancelled()? {
                        let _ = fs::remove_file(&part_path);
                        return Err(ModelAcquisitionError::Cancelled);
                    }
                    let read = response
                        .read(&mut buffer)
                        .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
                    if read == 0 {
                        break;
                    }
                    part_file
                        .write_all(&buffer[..read])
                        .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
                }
            } else {
                copy_local_file_to_part(Path::new(&url), &part_path)?;
            }

            fs::rename(&part_path, &target_path)
                .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
            let metadata = target_path
                .metadata()
                .map_err(|error| ModelAcquisitionError::Download(error.to_string()))?;
            files.push(DownloadedModelFile {
                relative_path: relative_path.clone(),
                absolute_path: target_path.clone(),
                size_bytes: metadata.len(),
                sha256: sha256_file(&target_path)?,
            });
            progress.file_completed(index as u32 + 1)?;
        }

        Ok(DownloadedModelArtifact {
            source: source.clone(),
            files,
        })
    }
```

- [ ] **Step 4: Run the download test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models hugging_face_provider_downloads_included_files_and_records_sha256 -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit provider download**

Run:

```bash
rtk git add src-tauri/src/transcription/acquisition.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: download model artifacts from providers"
```

Expected: commit succeeds.

## Task 5: Route Store Download Through Provider

**Files:**
- Modify: `src-tauri/src/transcription/store.rs`
- Test: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing store install test**

Append this test to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn store_download_installs_coreml_source_into_runtime_layout() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    write_minimal_coreml_source(source.path());

    let entry = parakeet_v3_catalog_entry();
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let status = store
        .download_from_local_source_for_test(entry.id, source.path())
        .expect("download from local source");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let runtime_dir = store
        .runtime_model_dir(entry.id, FLUID_AUDIO_COREML_RUNTIME_ID)
        .expect("runtime dir");
    assert!(runtime_dir.join("Encoder.mlmodelc/coremldata.bin").is_file());
    assert!(runtime_dir.join("JointDecisionv3.mlmodelc/coremldata.bin").is_file());
    assert!(runtime_dir.join("parakeet_v3_vocab.json").is_file());
    assert!(runtime_dir.join(COREML_INSPECTION_FILE_NAME).is_file());
}
```

Add this helper near the other model fixture helpers:

```rust
fn write_minimal_coreml_source(root: &Path) {
    for path in [
        "config.json",
        "parakeet_v3_vocab.json",
        "Encoder.mlmodelc/coremldata.bin",
        "Encoder.mlmodelc/metadata.json",
        "Encoder.mlmodelc/model.mil",
        "Encoder.mlmodelc/weights/weight.bin",
        "Decoder.mlmodelc/coremldata.bin",
        "Decoder.mlmodelc/metadata.json",
        "Decoder.mlmodelc/model.mil",
        "Decoder.mlmodelc/weights/weight.bin",
        "JointDecisionv3.mlmodelc/coremldata.bin",
        "JointDecisionv3.mlmodelc/metadata.json",
        "JointDecisionv3.mlmodelc/model.mil",
        "JointDecisionv3.mlmodelc/weights/weight.bin",
        "Preprocessor.mlmodelc/coremldata.bin",
        "Preprocessor.mlmodelc/metadata.json",
        "Preprocessor.mlmodelc/model.mil",
        "Preprocessor.mlmodelc/weights/weight.bin",
    ] {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&target, format!("fixture {path}")).expect("write fixture");
    }
}
```

- [ ] **Step 2: Run the failing store install test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_download_installs_coreml_source_into_runtime_layout -- --nocapture
```

Expected: FAIL because `download_from_local_source_for_test` does not exist and `download()` still rejects Core ML.

- [ ] **Step 3: Add provider-backed store install path**

In `src-tauri/src/transcription/store.rs`, import acquisition types:

```rust
use super::acquisition::{
    HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
};
```

Add a progress adapter:

```rust
struct StoreDownloadProgress<'a> {
    store: &'a TranscriptionModelStore,
    model_id: &'a str,
    token: DownloadToken,
}

impl ModelDownloadProgress for StoreDownloadProgress<'_> {
    fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
        self.store
            .download_is_cancelled_or_stale(self.model_id, self.token)
            .map_err(|error| ModelAcquisitionError::Download(error.to_string()))
    }

    fn file_completed(&self, downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
        self.store
            .update_download_progress_if_owner(self.model_id, self.token, downloaded_files)
            .map(|_| ())
            .map_err(|error| ModelAcquisitionError::Download(error.to_string()))
    }
}
```

Replace the Core ML rejection in `download()` with provider-backed install:

```rust
    pub fn download(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let total_files = entry
            .artifact_sources
            .first()
            .map(|source| source.include_files.len())
            .unwrap_or(entry.required_files.len()) as u32;
        let token = match self.mark_download_started(entry.id, total_files)? {
            StartDownload::Started { token, .. } => token,
            StartDownload::AlreadyActive(status) => return Ok(status),
        };

        let download_result = self.download_with_default_provider(&entry, token);
        self.remove_active_download_if_owner(entry.id, token)?;

        if !download_result? {
            return Ok(self.build_status(&entry, ModelInstallStatus::Failed, None));
        }

        self.verify(entry.id)
    }
```

Add these store methods:

```rust
    fn download_with_default_provider(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        token: DownloadToken,
    ) -> Result<bool, ModelStoreError> {
        let source = entry
            .artifact_sources
            .first()
            .ok_or_else(|| ModelStoreError::Download("model has no artifact source".to_string()))?;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
            .timeout(DOWNLOAD_REQUEST_TIMEOUT)
            .build()
            .map_err(|source| {
                ModelStoreError::Download(format!("failed to create download client: {source}"))
            })?;
        let provider = HuggingFaceHubProvider::new(client);
        provider
            .inspect_remote(source)
            .map_err(|error| ModelStoreError::Download(error.to_string()))?;
        self.download_with_provider(entry, source, &provider, token)
    }

    fn download_with_provider(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        source: &super::model::ModelArtifactSource,
        provider: &dyn ModelArtifactProvider,
        token: DownloadToken,
    ) -> Result<bool, ModelStoreError> {
        let staging_root = self.create_import_staging_root(entry)?;
        let _staging_cleanup = StagingDirCleanup {
            path: staging_root.clone(),
        };
        let staged_model_dir = staging_root.join(safe_model_dir_name(entry.id));
        let staged_runtime_dir = self.runtime_files_dir_for_entry(entry, &staged_model_dir);
        fs::create_dir_all(&staged_runtime_dir).map_err(|error| {
            ModelStoreError::Download(format!(
                "failed to create staged model directory {}: {error}",
                staged_runtime_dir.display()
            ))
        })?;

        let _artifact = match provider.download(
            source,
            &staged_runtime_dir,
            &StoreDownloadProgress {
                store: self,
                model_id: entry.id,
                token,
            },
        ) {
            Ok(artifact) => artifact,
            Err(ModelAcquisitionError::Cancelled) => return Ok(false),
            Err(error) => return Err(ModelStoreError::Download(error.to_string())),
        };
        validate_required_import_files(entry, &staged_runtime_dir)?;
        if self
            .write_verified_staged_manifest(entry, &staging_root)?
            .is_none()
        {
            return Ok(false);
        }
        let model_dir = self.model_dir_for_entry(entry);
        let backup_dir = self.import_backup_dir_for_entry(entry);
        replace_model_dir_with_staged(&model_dir, &staged_model_dir, &backup_dir)?;
        Ok(true)
    }
```

Add this test-support helper. Keep it public because integration tests compile the library as a dependency and cannot call `#[cfg(test)]` items:

```rust
    #[doc(hidden)]
    pub fn download_from_local_source_for_test(
        &self,
        model_id: &str,
        source_dir: &Path,
    ) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = self.catalog_entry(model_id)?;
        let mut source = entry
            .artifact_sources
            .first()
            .cloned()
            .ok_or_else(|| ModelStoreError::Download("model has no artifact source".to_string()))?;
        let total_files = source.include_files.len() as u32;
        let token = match self.mark_download_started(entry.id, total_files)? {
            StartDownload::Started { token, .. } => token,
            StartDownload::AlreadyActive(status) => return Ok(status),
        };
        source.repo_id = "local/test".to_string();
        source.revision = "fixture".to_string();
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|error| ModelStoreError::Download(error.to_string()))?;
        let provider = HuggingFaceHubProvider::with_base_urls(
            client,
            "http://127.0.0.1/unused",
            source_dir.to_string_lossy().to_string(),
        );
        let result = self.download_with_provider(&entry, &source, &provider, token);
        self.remove_active_download_if_owner(entry.id, token)?;
        if !result? {
            return Ok(self.build_status(&entry, ModelInstallStatus::Failed, None));
        }
        self.verify(entry.id)
    }
```

- [ ] **Step 4: Run the store install test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_download_installs_coreml_source_into_runtime_layout -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run related store tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models -- --nocapture
```

Expected: PASS. If old non-CoreML download tests fail because they assumed the old `entry.id` URL behavior, update them to assert source-provider behavior.

- [ ] **Step 6: Commit store download routing**

Run:

```bash
rtk git add src-tauri/src/transcription/store.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: install models from artifact providers"
```

Expected: commit succeeds.

## Task 6: Persist Source-Aware Installed Manifest

**Files:**
- Modify: `src-tauri/src/transcription/model.rs`
- Modify: `src-tauri/src/transcription/store.rs`
- Test: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing manifest source test**

Append this test to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn downloaded_model_manifest_records_artifact_source_and_file_hashes() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    write_minimal_coreml_source(source.path());

    let entry = parakeet_v3_catalog_entry();
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    store
        .download_from_local_source_for_test(entry.id, source.path())
        .expect("download fixture source");

    let manifest_path = root
        .path()
        .join(safe_model_dir_name(entry.id))
        .join(MODEL_MANIFEST_FILE_NAME);
    let manifest_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("manifest json");

    assert_eq!(manifest_json["modelId"], entry.id);
    assert_eq!(manifest_json["artifactFormat"], "core_ml_bundle");
    assert_eq!(manifest_json["modality"], "transcription");
    assert_eq!(manifest_json["source"]["provider"], "hugging_face_hub");
    assert_eq!(manifest_json["source"]["repoId"], "local/test");
    assert_eq!(manifest_json["source"]["revision"], "fixture");
    let installed_files = manifest_json["installedFiles"]
        .as_array()
        .expect("installed files");
    let encoder = installed_files
        .iter()
        .find(|file| file["path"] == "Encoder.mlmodelc/coremldata.bin")
        .expect("encoder file in manifest");
    assert!(encoder["sizeBytes"].as_u64().expect("size") > 0);
    assert_eq!(encoder["sha256"].as_str().expect("sha").len(), 64);
}
```

- [ ] **Step 2: Run failing manifest source test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models downloaded_model_manifest_records_artifact_source_and_file_hashes -- --nocapture
```

Expected: FAIL because the installed manifest only records model id, revision, installed files, and timestamps.

- [ ] **Step 3: Extend manifest schema**

In `src-tauri/src/transcription/model.rs`, update `InstalledModelManifest`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModelManifest {
    #[serde(default = "installed_model_manifest_schema_version")]
    pub schema_version: u32,
    pub model_id: String,
    pub revision: String,
    #[serde(default)]
    pub modality: Option<ModelModality>,
    #[serde(default)]
    pub artifact_format: Option<TranscriptionModelArtifactFormat>,
    #[serde(default)]
    pub source: Option<InstalledModelSource>,
    pub installed_files: Vec<TranscriptionModelFile>,
    pub installed_at: String,
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModelSource {
    pub provider: ModelArtifactProviderKind,
    pub repo_id: String,
    pub revision: String,
    pub license: Option<String>,
}

fn installed_model_manifest_schema_version() -> u32 {
    1
}
```

Update `validate_against` so old manifests still validate by only requiring `modality`, `artifact_format`, and `source` when present:

```rust
        if let Some(modality) = self.modality {
            if modality != entry.modality {
                return ModelInstallStatus::Failed;
            }
        }
        if let Some(artifact_format) = self.artifact_format {
            if artifact_format != entry.artifact_format {
                return ModelInstallStatus::Failed;
            }
        }
```

Update the two existing `InstalledModelManifest` struct literals in `src-tauri/tests/transcription_models.rs` so they include the new fields. For `manifest_validation_rejects_missing_required_files`, use:

```rust
    let manifest = InstalledModelManifest {
        schema_version: 1,
        model_id: entry.id.to_string(),
        revision: "main".to_string(),
        modality: None,
        artifact_format: None,
        source: None,
        installed_files: vec![TranscriptionModelFile {
            path: entry.required_files[0].path.clone(),
            size_bytes: Some(1),
            sha256: None,
        }],
        installed_at: "2026-06-13T00:00:00Z".to_string(),
        verified_at: None,
    };
```

For `complete_manifest`, use:

```rust
fn complete_manifest(entry: &TranscriptionModelCatalogEntry) -> InstalledModelManifest {
    InstalledModelManifest {
        schema_version: 1,
        model_id: entry.id.to_string(),
        revision: entry.revision.to_string(),
        modality: Some(entry.modality),
        artifact_format: Some(entry.artifact_format),
        source: None,
        installed_files: entry.required_files.clone(),
        installed_at: "2026-06-13T00:00:00Z".to_string(),
        verified_at: Some("2026-06-13T00:01:00Z".to_string()),
    }
}
```

- [ ] **Step 4: Write source-aware manifests from store**

In `src-tauri/src/transcription/store.rs`, update imports to include `InstalledModelSource`.

Add:

```rust
    fn ready_manifest_for_entry_with_source(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        source: Option<&super::model::ModelArtifactSource>,
        installed_files: Vec<super::model::TranscriptionModelFile>,
    ) -> InstalledModelManifest {
        let now = Utc::now().to_rfc3339();
        InstalledModelManifest {
            schema_version: 1,
            model_id: entry.id.to_string(),
            revision: entry.revision.to_string(),
            modality: Some(entry.modality),
            artifact_format: Some(entry.artifact_format),
            source: source.map(|source| InstalledModelSource {
                provider: source.provider,
                repo_id: source.repo_id.clone(),
                revision: source.revision.clone(),
                license: source.license.clone(),
            }),
            installed_files,
            installed_at: now.clone(),
            verified_at: Some(now),
        }
    }
```

Keep `ready_manifest_for_entry` as a wrapper:

```rust
        self.ready_manifest_for_entry_with_source(entry, None, entry.required_files.clone())
```

Change `download_with_provider` to capture the downloaded artifact and pass hashed file metadata into manifest writing:

```rust
        let artifact = provider
            .download(
                source,
                &staged_runtime_dir,
                &StoreDownloadProgress {
                    store: self,
                    model_id: entry.id,
                    token,
                },
            )
            .map_err(|error| ModelStoreError::Download(error.to_string()))?;
        let installed_files = artifact
            .files
            .into_iter()
            .map(|file| super::model::TranscriptionModelFile {
                path: file.relative_path,
                size_bytes: Some(file.size_bytes),
                sha256: Some(file.sha256),
            })
            .collect::<Vec<_>>();
```

Add a new staged manifest writer:

```rust
    fn write_verified_staged_manifest_with_source(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        staging_root: &Path,
        source: Option<&super::model::ModelArtifactSource>,
        installed_files: Vec<super::model::TranscriptionModelFile>,
    ) -> Result<Option<InstalledModelManifest>, ModelStoreError> {
        let manifest = self.ready_manifest_for_entry_with_source(entry, source, installed_files);
        if manifest.validate_against(entry, staging_root) != ModelInstallStatus::Ready {
            return Ok(None);
        }
        self.write_manifest_to_model_dir(
            &staging_root.join(safe_model_dir_name(entry.id)),
            &manifest,
        )?;
        self.write_coreml_inspection_summary_if_needed(
            entry,
            &staging_root.join(safe_model_dir_name(entry.id)),
        )?;
        Ok(Some(manifest))
    }
```

Call `write_verified_staged_manifest_with_source(entry, &staging_root, Some(source), installed_files)` from provider downloads. Leave import path on `write_verified_staged_manifest` with `source: None`.

- [ ] **Step 5: Run manifest source test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models downloaded_model_manifest_records_artifact_source_and_file_hashes -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Run model tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit source-aware manifests**

Run:

```bash
rtk git add src-tauri/src/transcription/model.rs src-tauri/src/transcription/store.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: record model source manifests"
```

Expected: commit succeeds.

## Task 7: Update Real E2E Bootstrap Path

**Files:**
- Modify: `src-tauri/tests/transcription_e2e.rs`

- [ ] **Step 1: Write failing E2E env test for download bootstrap**

Add this constant near the other E2E env constants:

```rust
const DOWNLOAD_MODEL_ENV: &str = "VIDEO_CREATER_TEST_DOWNLOAD_MODEL";
```

Add a field to `E2eEnvVars` and `E2eConfig`:

```rust
download_model: bool,
```

Append this test:

```rust
#[test]
fn transcription_e2e_download_mode_does_not_require_model_root() {
    let vars = E2eEnvVars {
        model_root: None,
        wav: Some(PathBuf::from("/tmp/audio.wav")),
        media: None,
        helper: None,
        download_model: true,
    };

    let E2eEnvState::Requested(config) = vars.into_state() else {
        panic!("expected requested download config");
    };

    assert!(config.download_model);
    assert_eq!(config.media_path, PathBuf::from("/tmp/audio.wav"));
}
```

Update existing `E2eEnvVars` struct literals in this file with `download_model: false`.

- [ ] **Step 2: Run failing E2E env test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e transcription_e2e_download_mode_does_not_require_model_root -- --nocapture
```

Expected: FAIL until env parsing supports download mode.

- [ ] **Step 3: Implement download-mode env parsing**

In `src-tauri/tests/transcription_e2e.rs`, update `from_process_env()`:

```rust
download_model: env_flag(DOWNLOAD_MODEL_ENV),
```

Add:

```rust
fn env_flag(name: &str) -> bool {
    env::var(name).ok().as_deref() == Some("1")
}
```

Update `into_state()` so `download_model == true` allows `model_root` to be absent but still requires media:

```rust
        let requested = self.model_root.is_some()
            || self.wav.is_some()
            || self.media.is_some()
            || self.helper.is_some()
            || self.download_model;
```

When validating model root:

```rust
        if !self.download_model && self.model_root.is_none() {
            return E2eEnvState::Misconfigured(format!(
                "set {MODEL_ROOT_ENV} or {DOWNLOAD_MODEL_ENV}=1"
            ));
        }
```

- [ ] **Step 4: Run E2E env tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e transcription_e2e_env -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Make real E2E install by download when requested**

In `run_real_fluidaudio_temporal_transcription_e2e()`, replace the unconditional import block:

```rust
    let status = if config.download_model {
        store.download(entry.id).expect("download FluidAudio/Core ML model")
    } else {
        let model_source = importable_model_source_dir(
            config
                .model_root
                .as_ref()
                .expect("model root required without download mode"),
        );
        store
            .import_model(entry.id, &model_source)
            .expect("import local FluidAudio/Core ML model")
    };
```

Update `E2eConfig` so `model_root` is `Option<PathBuf>`, and only assert `model_root.is_dir()` when present.

Update skip copy:

```rust
eprintln!(
    "skipping real FluidAudio transcription E2E: set {DOWNLOAD_MODEL_ENV}=1 or {MODEL_ROOT_ENV}, plus either {WAV_ENV} or {MEDIA_ENV}"
);
```

- [ ] **Step 6: Run E2E tests in skip mode**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e -- --nocapture
```

Expected: PASS. The real E2E still skips unless env vars are set.

- [ ] **Step 7: Commit E2E bootstrap update**

Run:

```bash
rtk git add src-tauri/tests/transcription_e2e.rs
rtk git commit -m "test: bootstrap transcription e2e model download"
```

Expected: commit succeeds.

## Task 8: Final Verification

**Files:**
- Verify all changed Rust files.

- [ ] **Step 1: Format check**

Run:

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS.

- [ ] **Step 2: Run transcription model tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Run transcription E2E tests in default skip mode**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e -- --nocapture
```

Expected: PASS. The real E2E should skip unless env vars are set.

- [ ] **Step 4: Run Temporal workflow tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run worker check**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker
```

Expected: PASS.

- [ ] **Step 6: Optional real macOS network E2E**

Only run when network, disk space, and helper binary are available:

```bash
VIDEO_CREATER_TEST_DOWNLOAD_MODEL=1 \
VIDEO_CREATER_TEST_TRANSCRIPTION_WAV=/absolute/path/to/speech.wav \
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_e2e real_fluidaudio_temporal_transcription_e2e -- --nocapture
```

Expected: PASS with non-empty transcript words and timings. If no speech fixture is available, report that this optional real E2E was not run.

- [ ] **Step 7: Final status check**

Run:

```bash
rtk git status --short
```

Expected: only intentional tracked changes are present; existing unrelated untracked paths such as `palmier-pro/` and `src-tauri/native/fluidaudio-parakeet/.build/` may remain.
