# Parakeet Model Manager And Rust Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a dedicated Parakeet v3 model manager, make transcription Rust-owned, and add a runtime abstraction that supports Python fallback and a native Rust capability slot.

**Architecture:** Rust owns model catalog, app-managed model paths, install state, verification, runtime selection, transcription job orchestration, and Generate edit gating. React renders a Settings / Models screen and invokes Tauri commands without deciding model paths or runtime availability. The native runtime starts as a probeable unsupported implementation behind the same interface used by the Python fallback.

**Tech Stack:** Rust, Tauri commands, serde, tempfile tests, React, TypeScript, Vitest, Testing Library, existing Python Parakeet runner.

---

## File Structure

- Create `src-tauri/src/transcription/mod.rs`: exports transcription model, store, runtime, and job modules.
- Create `src-tauri/src/transcription/model.rs`: catalog types, install states, manifest validation, path-safe model ids.
- Create `src-tauri/src/transcription/store.rs`: app-managed model root, status loading, verification, removal, and download state transitions.
- Create `src-tauri/src/transcription/runtime.rs`: `TranscriptionRuntime` trait, runtime capability types, selection logic, Python fallback runtime, native unsupported runtime.
- Create `src-tauri/src/transcription/job.rs`: transcription job input/output types and transcript mutation boundary.
- Modify `src-tauri/src/lib.rs`: export `transcription`.
- Modify `src-tauri/src/main.rs`: manage transcription state and expose model/runtime/job commands.
- Modify `src-tauri/src/edit/transcriber.rs`: switch Parakeet command construction from model id to local model directory.
- Modify `src-tauri/python/parakeet_runner.py`: accept `--model-path` and use local files only.
- Create `src-tauri/tests/transcription_models.rs`: Rust tests for catalog, manifest, store, runtime selection, and Generate edit gating.
- Create `src/lib/transcription-models.ts`: thin frontend Tauri adapter and frontend-facing types.
- Create `src/components/settings/model-settings.tsx`: Settings / Models screen.
- Create `src/components/settings/model-settings.test.tsx`: UI state tests.
- Modify `src/App.tsx`: simple in-app route state between editor and Settings / Models.
- Modify `src/components/workspace/editor-workspace.tsx`: add Settings button, pass model status into Agent panel.
- Modify `src/components/workspace/agent-panel.tsx`: block Generate edit when local model is missing and expose Settings action.
- Modify `src/components/workspace/agent-panel.test.tsx` if it exists; otherwise add `src/components/workspace/agent-panel.test.tsx`.
- Modify `src-tauri/Cargo.toml`: add the Rust download dependency in Task 5.

## Task 1: Rust Model Catalog And Manifest Validation

**Files:**
- Create: `src-tauri/src/transcription/mod.rs`
- Create: `src-tauri/src/transcription/model.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing Rust tests for catalog and manifest validation**

Create `src-tauri/tests/transcription_models.rs`:

```rust
use std::fs;

use tempfile::tempdir;
use video_creater_lib::transcription::model::{
    parakeet_v3_catalog_entry, InstalledModelManifest, ModelInstallStatus,
    TranscriptionModelFile,
};

#[test]
fn parakeet_v3_catalog_entry_lists_required_local_files() {
    let entry = parakeet_v3_catalog_entry();

    assert_eq!(entry.id, "nvidia/parakeet-tdt-0.6b-v3");
    assert_eq!(entry.display_name, "Parakeet TDT 0.6B v3");
    assert!(entry.approximate_size_bytes >= 5_000_000_000);
    assert!(entry.required_files.iter().any(|file| file.path == "model.safetensors"));
    assert!(entry.required_files.iter().any(|file| file.path == "tokenizer.json"));
    assert!(entry.required_files.iter().any(|file| file.path == "processor_config.json"));
    assert!(entry.required_files.iter().any(|file| file.path == "generation_config.json"));
}

#[test]
fn manifest_validation_rejects_missing_required_files() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    fs::create_dir_all(root.path().join("nvidia__parakeet-tdt-0.6b-v3"))
        .expect("model dir");

    let manifest = InstalledModelManifest {
        model_id: entry.id.to_string(),
        revision: "main".to_string(),
        installed_files: vec![TranscriptionModelFile {
            path: "model.safetensors".to_string(),
            size_bytes: Some(1),
            sha256: None,
        }],
        installed_at: "2026-06-13T00:00:00Z".to_string(),
        verified_at: None,
    };

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_accepts_complete_nonempty_install() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    fs::create_dir_all(&model_dir).expect("model dir");

    for file in &entry.required_files {
        fs::write(model_dir.join(&file.path), b"present").expect("write model file");
    }

    let manifest = InstalledModelManifest {
        model_id: entry.id.to_string(),
        revision: "main".to_string(),
        installed_files: entry.required_files.clone(),
        installed_at: "2026-06-13T00:00:00Z".to_string(),
        verified_at: Some("2026-06-13T00:01:00Z".to_string()),
    };

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Ready);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models
```

Expected: FAIL because `video_creater_lib::transcription` does not exist.

- [ ] **Step 3: Add the transcription module export**

Modify `src-tauri/src/lib.rs`:

```rust
pub mod codex;
pub mod edit;
pub mod project;
pub mod transcription;
```

Create `src-tauri/src/transcription/mod.rs`:

```rust
pub mod model;
```

- [ ] **Step 4: Implement catalog and manifest types**

Create `src-tauri/src/transcription/model.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionModelCatalogEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub revision: &'static str,
    pub approximate_size_bytes: u64,
    pub required_files: Vec<TranscriptionModelFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionModelFile {
    pub path: String,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelInstallStatus {
    Missing,
    Downloading,
    Verifying,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModelManifest {
    pub model_id: String,
    pub revision: String,
    pub installed_files: Vec<TranscriptionModelFile>,
    pub installed_at: String,
    pub verified_at: Option<String>,
}

pub fn parakeet_v3_catalog_entry() -> TranscriptionModelCatalogEntry {
    TranscriptionModelCatalogEntry {
        id: "nvidia/parakeet-tdt-0.6b-v3",
        display_name: "Parakeet TDT 0.6B v3",
        provider: "NVIDIA",
        revision: "main",
        approximate_size_bytes: 5_020_000_000,
        required_files: vec![
            required_file("model.safetensors"),
            required_file("tokenizer.json"),
            required_file("tokenizer_config.json"),
            required_file("config.json"),
            required_file("processor_config.json"),
            required_file("generation_config.json"),
        ],
    }
}

pub fn safe_model_dir_name(model_id: &str) -> String {
    model_id.replace('/', "__")
}

impl InstalledModelManifest {
    pub fn validate_against(
        &self,
        entry: &TranscriptionModelCatalogEntry,
        model_root: &Path,
    ) -> ModelInstallStatus {
        if self.model_id != entry.id || self.revision != entry.revision {
            return ModelInstallStatus::Failed;
        }

        let model_dir = model_root.join(safe_model_dir_name(entry.id));
        for required in &entry.required_files {
            if !self
                .installed_files
                .iter()
                .any(|installed| installed.path == required.path)
            {
                return ModelInstallStatus::Failed;
            }

            let path = model_dir.join(&required.path);
            let Ok(metadata) = path.metadata() else {
                return ModelInstallStatus::Failed;
            };
            if metadata.len() == 0 {
                return ModelInstallStatus::Failed;
            }
        }

        if self.verified_at.is_some() {
            ModelInstallStatus::Ready
        } else {
            ModelInstallStatus::Verifying
        }
    }
}

fn required_file(path: &str) -> TranscriptionModelFile {
    TranscriptionModelFile {
        path: path.to_string(),
        size_bytes: None,
        sha256: None,
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models parakeet_v3_catalog_entry_lists_required_local_files manifest_validation_
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
rtk git add src-tauri/src/lib.rs src-tauri/src/transcription/mod.rs src-tauri/src/transcription/model.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add parakeet model catalog"
```

## Task 2: Model Store Status, Verification, And Removal

**Files:**
- Create: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/src/transcription/mod.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Add failing store tests**

Append to `src-tauri/tests/transcription_models.rs`:

```rust
use video_creater_lib::transcription::model::safe_model_dir_name;
use video_creater_lib::transcription::store::TranscriptionModelStore;

#[test]
fn store_reports_missing_when_manifest_is_absent() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());

    let status = store.status("nvidia/parakeet-tdt-0.6b-v3").expect("status");

    assert_eq!(status.install_status, ModelInstallStatus::Missing);
    assert_eq!(status.model_id, "nvidia/parakeet-tdt-0.6b-v3");
}

#[test]
fn store_verifies_complete_install_and_writes_manifest() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    for file in &entry.required_files {
        fs::write(model_dir.join(&file.path), b"present").expect("write model file");
    }

    let status = store.verify(entry.id).expect("verify");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    assert!(model_dir.join("video-creater-model-manifest.json").exists());
}

#[test]
fn store_remove_deletes_model_directory() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    fs::write(model_dir.join("model.safetensors"), b"present").expect("write model file");

    let status = store.remove(entry.id).expect("remove");

    assert_eq!(status.install_status, ModelInstallStatus::Missing);
    assert!(!model_dir.exists());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_
```

Expected: FAIL because `transcription::store` does not exist.

- [ ] **Step 3: Export and implement the store**

Modify `src-tauri/src/transcription/mod.rs`:

```rust
pub mod model;
pub mod store;
```

Create `src-tauri/src/transcription/store.rs`:

```rust
use super::model::{
    parakeet_v3_catalog_entry, safe_model_dir_name, InstalledModelManifest, ModelInstallStatus,
    TranscriptionModelCatalogEntry,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

const MANIFEST_FILE: &str = "video-creater-model-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionModelStatus {
    pub model_id: String,
    pub display_name: String,
    pub install_status: ModelInstallStatus,
    pub local_path: String,
    pub approximate_size_bytes: u64,
    pub verified_at: Option<String>,
}

#[derive(Debug, Error)]
pub enum ModelStoreError {
    #[error("unsupported transcription model: {0}")]
    UnsupportedModel(String),
    #[error("model io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("model manifest json failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct TranscriptionModelStore {
    root: PathBuf,
}

impl TranscriptionModelStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn list(&self) -> Result<Vec<TranscriptionModelStatus>, ModelStoreError> {
        Ok(vec![self.status(parakeet_v3_catalog_entry().id)?])
    }

    pub fn status(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = catalog_entry(model_id)?;
        let model_dir = self.model_dir(entry.id);
        let manifest_path = model_dir.join(MANIFEST_FILE);
        let install_status;
        let mut verified_at = None;

        if manifest_path.exists() {
            let manifest: InstalledModelManifest =
                serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
            install_status = manifest.validate_against(&entry, &self.root);
            verified_at = manifest.verified_at;
        } else if model_dir.exists() {
            install_status = ModelInstallStatus::Failed;
        } else {
            install_status = ModelInstallStatus::Missing;
        }

        Ok(status_from_entry(entry, model_dir, install_status, verified_at))
    }

    pub fn verify(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = catalog_entry(model_id)?;
        let model_dir = self.model_dir(entry.id);
        fs::create_dir_all(&model_dir)?;
        let now = Utc::now().to_rfc3339();
        let manifest = InstalledModelManifest {
            model_id: entry.id.to_string(),
            revision: entry.revision.to_string(),
            installed_files: entry.required_files.clone(),
            installed_at: now.clone(),
            verified_at: Some(now),
        };
        let install_status = manifest.validate_against(&entry, &self.root);
        fs::write(
            model_dir.join(MANIFEST_FILE),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        Ok(status_from_entry(
            entry,
            model_dir,
            install_status,
            manifest.verified_at,
        ))
    }

    pub fn remove(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = catalog_entry(model_id)?;
        let model_dir = self.model_dir(entry.id);
        if model_dir.exists() {
            fs::remove_dir_all(&model_dir)?;
        }

        Ok(status_from_entry(
            entry,
            model_dir,
            ModelInstallStatus::Missing,
            None,
        ))
    }

    pub fn model_dir(&self, model_id: &str) -> PathBuf {
        self.root.join(safe_model_dir_name(model_id))
    }
}

fn catalog_entry(model_id: &str) -> Result<TranscriptionModelCatalogEntry, ModelStoreError> {
    let entry = parakeet_v3_catalog_entry();
    if model_id == entry.id {
        Ok(entry)
    } else {
        Err(ModelStoreError::UnsupportedModel(model_id.to_string()))
    }
}

fn status_from_entry(
    entry: TranscriptionModelCatalogEntry,
    local_path: PathBuf,
    install_status: ModelInstallStatus,
    verified_at: Option<String>,
) -> TranscriptionModelStatus {
    TranscriptionModelStatus {
        model_id: entry.id.to_string(),
        display_name: entry.display_name.to_string(),
        install_status,
        local_path: local_path.display().to_string(),
        approximate_size_bytes: entry.approximate_size_bytes,
        verified_at,
    }
}
```

- [ ] **Step 4: Run store tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
rtk git add src-tauri/src/transcription/mod.rs src-tauri/src/transcription/store.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add transcription model store"
```

## Task 3: Runtime Abstraction And Python Local Model Path

**Files:**
- Create: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/src/transcription/mod.rs`
- Modify: `src-tauri/src/edit/transcriber.rs`
- Modify: `src-tauri/python/parakeet_runner.py`
- Modify: `src-tauri/tests/transcription_models.rs`
- Modify: `src-tauri/tests/one_click_edit.rs`

- [ ] **Step 1: Add failing runtime selection tests**

Append to `src-tauri/tests/transcription_models.rs`:

```rust
use video_creater_lib::transcription::runtime::{
    select_transcription_runtime, NativeParakeetRuntime, PythonParakeetRuntime,
    RuntimeCapability, RuntimeSelection,
};

#[test]
fn runtime_selection_prefers_native_when_ready() {
    let selection = select_transcription_runtime(
        RuntimeCapability::Ready,
        RuntimeCapability::Ready,
    );

    assert_eq!(selection, RuntimeSelection::Native);
}

#[test]
fn runtime_selection_uses_python_when_native_is_unsupported() {
    let selection = select_transcription_runtime(
        RuntimeCapability::Unsupported,
        RuntimeCapability::Ready,
    );

    assert_eq!(selection, RuntimeSelection::Python);
}

#[test]
fn runtime_selection_reports_unavailable_when_no_runtime_can_run() {
    let selection = select_transcription_runtime(
        RuntimeCapability::Unsupported,
        RuntimeCapability::Unavailable,
    );

    assert_eq!(selection, RuntimeSelection::Unavailable);
}

#[test]
fn native_runtime_starts_as_unsupported() {
    let native = NativeParakeetRuntime;

    assert_eq!(native.probe(), RuntimeCapability::Unsupported);
}

#[test]
fn python_runtime_is_ready_when_model_dir_and_runner_exist() {
    let root = tempdir().expect("temp model root");
    let model_dir = root.path().join("model");
    fs::create_dir_all(&model_dir).expect("model dir");
    let runner = root.path().join("parakeet_runner.py");
    fs::write(&runner, "print('runner')").expect("runner");
    let runtime = PythonParakeetRuntime::new("python3".to_string(), runner, model_dir);

    assert_eq!(runtime.probe(), RuntimeCapability::Ready);
}
```

- [ ] **Step 2: Add failing command construction test**

Modify the existing `build_parakeet_command` test in `src-tauri/tests/one_click_edit.rs` or add this test near the transcriber tests:

```rust
#[test]
fn parakeet_command_uses_local_model_path() {
    let config = ParakeetRunnerConfig {
        python_path: "python3".to_string(),
        runner_script_path: "src-tauri/python/parakeet_runner.py".to_string(),
        model_path: "/app/models/nvidia__parakeet-tdt-0.6b-v3".to_string(),
        audio_options: ParakeetAudioOptions::voiceink_style(),
    };
    let job = ParakeetTranscriptionJob {
        media_id: "media-1".to_string(),
        wav_path: "/tmp/input.wav".to_string(),
        output_json_path: "/tmp/transcript.json".to_string(),
        language_mode: LanguageMode::English,
    };

    let command = build_parakeet_command(&config, &job);

    assert!(command.args.contains(&"--model-path".to_string()));
    assert!(command
        .args
        .contains(&"/app/models/nvidia__parakeet-tdt-0.6b-v3".to_string()));
    assert!(!command.args.contains(&"--model-id".to_string()));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml runtime_selection_ native_runtime_ python_runtime_ parakeet_command_uses_local_model_path
```

Expected: FAIL because runtime module and `model_path` do not exist.

- [ ] **Step 4: Implement runtime abstraction**

Modify `src-tauri/src/transcription/mod.rs`:

```rust
pub mod model;
pub mod runtime;
pub mod store;
```

Create `src-tauri/src/transcription/runtime.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapability {
    Ready,
    Unsupported,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSelection {
    Native,
    Python,
    Unavailable,
}

pub trait TranscriptionRuntime {
    fn id(&self) -> &'static str;
    fn probe(&self) -> RuntimeCapability;
}

pub struct NativeParakeetRuntime;

impl TranscriptionRuntime for NativeParakeetRuntime {
    fn id(&self) -> &'static str {
        "native_parakeet"
    }

    fn probe(&self) -> RuntimeCapability {
        RuntimeCapability::Unsupported
    }
}

#[derive(Debug, Clone)]
pub struct PythonParakeetRuntime {
    python_path: String,
    runner_script_path: PathBuf,
    model_dir: PathBuf,
}

impl PythonParakeetRuntime {
    pub fn new(python_path: String, runner_script_path: PathBuf, model_dir: PathBuf) -> Self {
        Self {
            python_path,
            runner_script_path,
            model_dir,
        }
    }
}

impl TranscriptionRuntime for PythonParakeetRuntime {
    fn id(&self) -> &'static str {
        "python_parakeet"
    }

    fn probe(&self) -> RuntimeCapability {
        if self.python_path.trim().is_empty() {
            return RuntimeCapability::Unavailable;
        }
        if !self.runner_script_path.exists() || !self.model_dir.exists() {
            return RuntimeCapability::Unavailable;
        }
        RuntimeCapability::Ready
    }
}

pub fn select_transcription_runtime(
    native: RuntimeCapability,
    python: RuntimeCapability,
) -> RuntimeSelection {
    match (native, python) {
        (RuntimeCapability::Ready, _) => RuntimeSelection::Native,
        (_, RuntimeCapability::Ready) => RuntimeSelection::Python,
        _ => RuntimeSelection::Unavailable,
    }
}
```

- [ ] **Step 5: Switch transcriber config to local model path**

Modify `src-tauri/src/edit/transcriber.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParakeetRunnerConfig {
    pub python_path: String,
    pub runner_script_path: String,
    pub model_path: String,
    pub audio_options: ParakeetAudioOptions,
}
```

In `build_parakeet_command`, replace the `--model-id` pair:

```rust
"--model-path".to_string(),
config.model_path.clone(),
```

Keep all other arguments unchanged.

- [ ] **Step 6: Update tests using `ParakeetRunnerConfig`**

Search:

```bash
rtk rg "model_id|modelId|ParakeetRunnerConfig" src-tauri/tests src-tauri/src -n
```

For every Rust construction of `ParakeetRunnerConfig`, use:

```rust
model_path: "/app/models/nvidia__parakeet-tdt-0.6b-v3".to_string(),
```

- [ ] **Step 7: Update Python runner to use local files only**

Modify `src-tauri/python/parakeet_runner.py`:

```python
parser.add_argument("--model-path", required=True)
```

Remove the required `--model-id` argument. Replace model loading with:

```python
processor = AutoProcessor.from_pretrained(args.model_path, local_files_only=True)
model = AutoModelForTDT.from_pretrained(args.model_path, local_files_only=True)
```

Set the output payload model value to the local path:

```python
"model": args.model_path,
```

- [ ] **Step 8: Run runtime and command tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml runtime_selection_ native_runtime_ python_runtime_ parakeet_command_uses_local_model_path
```

Expected: PASS.

- [ ] **Step 9: Commit Task 3**

Run:

```bash
rtk git add src-tauri/src/transcription/mod.rs src-tauri/src/transcription/runtime.rs src-tauri/src/edit/transcriber.rs src-tauri/python/parakeet_runner.py src-tauri/tests/transcription_models.rs src-tauri/tests/one_click_edit.rs
rtk git commit -m "feat: add transcription runtime abstraction"
```

## Task 4: Tauri Commands And Generate Edit Gating

**Files:**
- Create: `src-tauri/src/transcription/job.rs`
- Modify: `src-tauri/src/transcription/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Add failing Generate edit gate unit tests**

Append to `src-tauri/tests/transcription_models.rs`:

```rust
use video_creater_lib::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::transcription::job::{
    validate_transcription_ready_for_generate_edit, TranscriptionJobError,
};

#[test]
fn generate_edit_gate_blocks_missing_model_when_project_has_no_transcript() {
    let project = sample_project();
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a trailer".to_string(),
        target_duration_seconds: None,
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-13T00:00:00Z".to_string(),
    };

    let error = validate_transcription_ready_for_generate_edit(
        &project,
        &request,
        ModelInstallStatus::Missing,
        RuntimeSelection::Unavailable,
    )
    .expect_err("missing model should block");

    assert_eq!(error, TranscriptionJobError::ModelNotReady);
}

#[test]
fn generate_edit_gate_allows_existing_transcript_without_ready_model() {
    let mut project = sample_project();
    project.transcripts.push(video_creater_lib::project::model::Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: None,
        segments: Vec::new(),
        words: Vec::new(),
    });
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a trailer".to_string(),
        target_duration_seconds: None,
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-13T00:00:00Z".to_string(),
    };

    validate_transcription_ready_for_generate_edit(
        &project,
        &request,
        ModelInstallStatus::Missing,
        RuntimeSelection::Unavailable,
    )
    .expect("existing transcript should allow edit generation");
}

#[test]
fn generate_edit_gate_allows_ready_model_and_runtime() {
    let project = sample_project();
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a trailer".to_string(),
        target_duration_seconds: None,
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-13T00:00:00Z".to_string(),
    };

    validate_transcription_ready_for_generate_edit(
        &project,
        &request,
        ModelInstallStatus::Ready,
        RuntimeSelection::Python,
    )
    .expect("ready model should allow transcription");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models generate_edit_gate_
```

Expected: FAIL because `transcription::job` does not exist.

- [ ] **Step 3: Implement job validation**

Modify `src-tauri/src/transcription/mod.rs`:

```rust
pub mod job;
pub mod model;
pub mod runtime;
pub mod store;
```

Create `src-tauri/src/transcription/job.rs`:

```rust
use super::model::ModelInstallStatus;
use super::runtime::RuntimeSelection;
use crate::edit::preset::EditJobRequest;
use crate::project::model::VideoProject;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TranscriptionJobError {
    #[error("local transcription model is not ready")]
    ModelNotReady,
    #[error("no transcription runtime is available")]
    RuntimeUnavailable,
}

pub fn validate_transcription_ready_for_generate_edit(
    project: &VideoProject,
    request: &EditJobRequest,
    install_status: ModelInstallStatus,
    runtime_selection: RuntimeSelection,
) -> Result<(), TranscriptionJobError> {
    let has_transcript = project
        .transcripts
        .iter()
        .any(|transcript| transcript.media_id == request.media_id);

    if has_transcript {
        return Ok(());
    }

    if install_status != ModelInstallStatus::Ready {
        return Err(TranscriptionJobError::ModelNotReady);
    }

    if runtime_selection == RuntimeSelection::Unavailable {
        return Err(TranscriptionJobError::RuntimeUnavailable);
    }

    Ok(())
}
```

- [ ] **Step 4: Add Tauri model state and commands**

Modify `src-tauri/src/main.rs` imports:

```rust
use std::sync::Mutex;
use video_creater_lib::transcription::model::parakeet_v3_catalog_entry;
use video_creater_lib::transcription::runtime::{
    select_transcription_runtime, NativeParakeetRuntime, PythonParakeetRuntime, RuntimeCapability,
    RuntimeSelection, TranscriptionRuntime,
};
use video_creater_lib::transcription::store::{TranscriptionModelStatus, TranscriptionModelStore};
```

Add state:

```rust
struct TranscriptionModelStoreState(Mutex<TranscriptionModelStore>);
```

Add commands:

```rust
#[tauri::command]
fn list_transcription_models(
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<Vec<TranscriptionModelStatus>, String> {
    state.0.lock().map_err(|_| "model store lock failed".to_string())?
        .list()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn verify_transcription_model(
    state: tauri::State<'_, TranscriptionModelStoreState>,
    model_id: String,
) -> Result<TranscriptionModelStatus, String> {
    state.0.lock().map_err(|_| "model store lock failed".to_string())?
        .verify(&model_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_transcription_model(
    state: tauri::State<'_, TranscriptionModelStoreState>,
    model_id: String,
) -> Result<TranscriptionModelStatus, String> {
    state.0.lock().map_err(|_| "model store lock failed".to_string())?
        .remove(&model_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_transcription_runtime_status(
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<RuntimeSelection, String> {
    let store = state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?;
    let entry = parakeet_v3_catalog_entry();
    let model_dir = store.model_dir(entry.id);
    let native = NativeParakeetRuntime;
    let runner_path = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join("src-tauri/python/parakeet_runner.py");
    let python = PythonParakeetRuntime::new("python3".to_string(), runner_path, model_dir);

    Ok(select_transcription_runtime(native.probe(), python.probe()))
}

#[tauri::command]
fn download_transcription_model(
    state: tauri::State<'_, TranscriptionModelStoreState>,
    model_id: String,
) -> Result<TranscriptionModelStatus, String> {
    state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?
        .status(&model_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_model_download(
    state: tauri::State<'_, TranscriptionModelStoreState>,
    model_id: String,
) -> Result<TranscriptionModelStatus, String> {
    state
        .0
        .lock()
        .map_err(|_| "model store lock failed".to_string())?
        .status(&model_id)
        .map_err(|error| error.to_string())
}
```

The download and cancel commands return current status in this task. Task 5 replaces the download body with real download transitions and keeps this command contract stable.

- [ ] **Step 5: Initialize store in Tauri setup and register commands**

In `.setup`, initialize the store:

```rust
.setup(|app| {
    app.manage("video-creater");
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
    app.manage(TranscriptionModelStoreState(Mutex::new(
        TranscriptionModelStore::new(app_data_dir.join("models")),
    )));
    Ok(())
})
```

Add commands to `tauri::generate_handler!`:

```rust
list_transcription_models,
download_transcription_model,
cancel_model_download,
verify_transcription_model,
remove_transcription_model,
get_transcription_runtime_status,
```

- [ ] **Step 6: Run Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models generate_edit_gate_
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 7: Commit Task 4**

Run:

```bash
rtk git add src-tauri/src/transcription/mod.rs src-tauri/src/transcription/job.rs src-tauri/src/main.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: gate transcription on local model readiness"
```

## Task 5: Real Download State, Progress, And Cancellation

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Add dependency entries**

Modify `src-tauri/Cargo.toml` dependencies:

```toml
reqwest = { version = "0.12.12", default-features = false, features = ["blocking", "rustls-tls"] }
```

- [ ] **Step 2: Add failing download state tests without network**

Append to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn store_marks_incomplete_download_as_failed_until_verified() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    fs::write(model_dir.join("model.safetensors"), b"partial").expect("partial");

    let status = store.status(entry.id).expect("status");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
}

#[test]
fn store_tracks_and_cancels_active_download() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();

    let downloading = store
        .mark_download_started(entry.id, 6)
        .expect("download started");

    assert_eq!(downloading.install_status, ModelInstallStatus::Downloading);
    assert_eq!(downloading.downloaded_files, 0);
    assert_eq!(downloading.total_files, 6);

    let cancelled = store.cancel_download(entry.id).expect("cancel download");

    assert_eq!(cancelled.install_status, ModelInstallStatus::Failed);
}
```

- [ ] **Step 3: Run test to verify current behavior**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_marks_incomplete_download_as_failed_until_verified
```

Expected: FAIL because active download tracking fields and methods do not exist.

- [ ] **Step 4: Extend status and store with active download tracking**

In `src-tauri/src/transcription/store.rs`, extend imports:

```rust
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
```

Add fields to `TranscriptionModelStatus`:

```rust
pub downloaded_files: u32,
pub total_files: u32,
```

Add active download tracking types:

```rust
#[derive(Debug)]
struct ActiveDownload {
    cancel: Arc<AtomicBool>,
    downloaded_files: u32,
    total_files: u32,
}
```

Change the store:

```rust
#[derive(Debug, Clone)]
pub struct TranscriptionModelStore {
    root: PathBuf,
    active_downloads: Arc<Mutex<BTreeMap<String, ActiveDownload>>>,
}
```

Update constructor:

```rust
pub fn new(root: PathBuf) -> Self {
    Self {
        root,
        active_downloads: Arc::new(Mutex::new(BTreeMap::new())),
    }
}
```

Update `status_from_entry` calls to include `downloaded_files: 0` and `total_files: entry.required_files.len() as u32`. Before returning missing/failed/ready status from `status`, check active downloads:

```rust
if let Some(active) = self
    .active_downloads
    .lock()
    .map_err(|_| ModelStoreError::Download("download state lock failed".to_string()))?
    .get(entry.id)
{
    return Ok(status_from_entry(
        entry,
        model_dir,
        ModelInstallStatus::Downloading,
        None,
        active.downloaded_files,
        active.total_files,
    ));
}
```

Add explicit transition methods used by tests and commands:

```rust
pub fn mark_download_started(
    &self,
    model_id: &str,
    total_files: u32,
) -> Result<TranscriptionModelStatus, ModelStoreError> {
    let entry = catalog_entry(model_id)?;
    let cancel = Arc::new(AtomicBool::new(false));
    self.active_downloads
        .lock()
        .map_err(|_| ModelStoreError::Download("download state lock failed".to_string()))?
        .insert(
            entry.id.to_string(),
            ActiveDownload {
                cancel,
                downloaded_files: 0,
                total_files,
            },
        );
    Ok(status_from_entry(
        entry,
        self.model_dir(model_id),
        ModelInstallStatus::Downloading,
        None,
        0,
        total_files,
    ))
}

pub fn cancel_download(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
    let entry = catalog_entry(model_id)?;
    if let Some(active) = self
        .active_downloads
        .lock()
        .map_err(|_| ModelStoreError::Download("download state lock failed".to_string()))?
        .remove(entry.id)
    {
        active.cancel.store(true, Ordering::SeqCst);
    }
    Ok(status_from_entry(
        entry,
        self.model_dir(model_id),
        ModelInstallStatus::Failed,
        None,
        0,
        entry.required_files.len() as u32,
    ))
}
```

- [ ] **Step 5: Add download method to store**

In `src-tauri/src/transcription/store.rs`, add:

```rust
impl TranscriptionModelStore {
    pub fn download(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError> {
        let entry = catalog_entry(model_id)?;
        let model_dir = self.model_dir(entry.id);
        fs::create_dir_all(&model_dir)?;

        let cancel = {
            let mut active_downloads = self
                .active_downloads
                .lock()
                .map_err(|_| ModelStoreError::Download("download state lock failed".to_string()))?;
            active_downloads
                .entry(entry.id.to_string())
                .or_insert_with(|| ActiveDownload {
                    cancel: Arc::new(AtomicBool::new(false)),
                    downloaded_files: 0,
                    total_files: entry.required_files.len() as u32,
                })
                .cancel
                .clone()
        };

        for (index, file) in entry.required_files.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                self.active_downloads
                    .lock()
                    .map_err(|_| ModelStoreError::Download("download state lock failed".to_string()))?
                    .remove(entry.id);
                return Ok(status_from_entry(
                    entry,
                    model_dir,
                    ModelInstallStatus::Failed,
                    None,
                    index as u32,
                    entry.required_files.len() as u32,
                ));
            }

            let url = format!(
                "https://huggingface.co/{}/resolve/{}/{}",
                entry.id, entry.revision, file.path
            );
            let part_path = model_dir.join(format!("{}.part", file.path));
            let final_path = model_dir.join(&file.path);
            let response = reqwest::blocking::get(&url)
                .map_err(|error| ModelStoreError::Download(error.to_string()))?;
            if !response.status().is_success() {
                return Err(ModelStoreError::Download(format!(
                    "download failed for {}: {}",
                    file.path,
                    response.status()
                )));
            }
            let bytes = response
                .bytes()
                .map_err(|error| ModelStoreError::Download(error.to_string()))?;
            fs::write(&part_path, &bytes)?;
            fs::rename(part_path, final_path)?;
            if let Some(active) = self
                .active_downloads
                .lock()
                .map_err(|_| ModelStoreError::Download("download state lock failed".to_string()))?
                .get_mut(entry.id)
            {
                active.downloaded_files = (index + 1) as u32;
            }
        }

        self.active_downloads
            .lock()
            .map_err(|_| ModelStoreError::Download("download state lock failed".to_string()))?
            .remove(entry.id);
        self.verify(model_id)
    }
}
```

Extend `ModelStoreError`:

```rust
#[error("model download failed: {0}")]
Download(String),
```

- [ ] **Step 6: Wire download and cancel commands**

In `src-tauri/src/main.rs`, replace `download_transcription_model` body with:

```rust
state
    .0
    .lock()
    .map_err(|_| "model store lock failed".to_string())?
    .download(&model_id)
    .map_err(|error| error.to_string())
```

Replace `cancel_model_download` with:

```rust
state
    .0
    .lock()
    .map_err(|_| "model store lock failed".to_string())?
    .cancel_download(&model_id)
    .map_err(|error| error.to_string())
```

- [ ] **Step 7: Run Rust tests without downloading**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models
```

Expected: PASS. The test suite must not call the real network download method.

- [ ] **Step 8: Commit Task 5**

Run:

```bash
rtk git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/transcription/store.rs src-tauri/src/main.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add parakeet model download flow"
```

## Task 6: Frontend Tauri Adapter And Settings / Models Screen

**Files:**
- Create: `src/lib/transcription-models.ts`
- Create: `src/components/settings/model-settings.tsx`
- Create: `src/components/settings/model-settings.test.tsx`

- [ ] **Step 1: Add failing UI tests for model states**

Create `src/components/settings/model-settings.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ModelSettings } from "./model-settings";
import type { TranscriptionModelStatus } from "@/lib/transcription-models";

function modelStatus(installStatus: TranscriptionModelStatus["installStatus"]): TranscriptionModelStatus {
  return {
    modelId: "nvidia/parakeet-tdt-0.6b-v3",
    displayName: "Parakeet TDT 0.6B v3",
    installStatus,
    localPath: "/app/models/nvidia__parakeet-tdt-0.6b-v3",
    approximateSizeBytes: 5_020_000_000,
    verifiedAt: installStatus === "ready" ? "2026-06-13T00:00:00Z" : null,
  };
}

describe("ModelSettings", () => {
  it("shows download action when Parakeet is missing", () => {
    render(
      <ModelSettings
        models={[modelStatus("missing")]}
        runtimeSelection="unavailable"
        onDownload={vi.fn()}
        onVerify={vi.fn()}
        onRemove={vi.fn()}
        onBack={vi.fn()}
      />,
    );

    expect(screen.getByText("Parakeet TDT 0.6B v3")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Download" })).toBeInTheDocument();
  });

  it("shows ready runtime status when Parakeet is installed", () => {
    render(
      <ModelSettings
        models={[modelStatus("ready")]}
        runtimeSelection="python"
        onDownload={vi.fn()}
        onVerify={vi.fn()}
        onRemove={vi.fn()}
        onBack={vi.fn()}
      />,
    );

    expect(screen.getByText("Ready")).toBeInTheDocument();
    expect(screen.getByText("Python fallback")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument();
  });

  it("shows repair action when verification failed", () => {
    render(
      <ModelSettings
        models={[modelStatus("failed")]}
        runtimeSelection="unavailable"
        onDownload={vi.fn()}
        onVerify={vi.fn()}
        onRemove={vi.fn()}
        onBack={vi.fn()}
      />,
    );

    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Verify" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk pnpm test -- src/components/settings/model-settings.test.tsx
```

Expected: FAIL because `ModelSettings` and the adapter types do not exist.

- [ ] **Step 3: Create frontend adapter types and commands**

Create `src/lib/transcription-models.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export type ModelInstallStatus = "missing" | "downloading" | "verifying" | "ready" | "failed";
export type RuntimeSelection = "native" | "python" | "unavailable";

export interface TranscriptionModelStatus {
  modelId: string;
  displayName: string;
  installStatus: ModelInstallStatus;
  localPath: string;
  approximateSizeBytes: number;
  verifiedAt: string | null;
}

export async function listTranscriptionModels(): Promise<TranscriptionModelStatus[]> {
  return invoke("list_transcription_models");
}

export async function downloadTranscriptionModel(modelId: string): Promise<TranscriptionModelStatus> {
  return invoke("download_transcription_model", { modelId });
}

export async function verifyTranscriptionModel(modelId: string): Promise<TranscriptionModelStatus> {
  return invoke("verify_transcription_model", { modelId });
}

export async function removeTranscriptionModel(modelId: string): Promise<TranscriptionModelStatus> {
  return invoke("remove_transcription_model", { modelId });
}

export async function getTranscriptionRuntimeStatus(): Promise<RuntimeSelection> {
  return invoke("get_transcription_runtime_status");
}
```

- [ ] **Step 4: Create settings screen component**

Create `src/components/settings/model-settings.tsx`:

```tsx
import { ArrowLeft, CheckCircle2, Download, RefreshCw, Trash2, XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { RuntimeSelection, TranscriptionModelStatus } from "@/lib/transcription-models";

interface ModelSettingsProps {
  models: TranscriptionModelStatus[];
  runtimeSelection: RuntimeSelection;
  onDownload: (modelId: string) => void;
  onVerify: (modelId: string) => void;
  onRemove: (modelId: string) => void;
  onBack: () => void;
}

export function ModelSettings({
  models,
  runtimeSelection,
  onDownload,
  onVerify,
  onRemove,
  onBack,
}: ModelSettingsProps) {
  const model = models[0];

  return (
    <main className="flex h-full min-h-0 flex-col bg-background">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b bg-card px-4">
        <Button variant="ghost" size="icon" aria-label="Back to editor" onClick={onBack}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div>
          <h1 className="text-sm font-semibold tracking-normal">Settings</h1>
          <p className="text-xs text-muted-foreground">Models</p>
        </div>
      </header>
      <section className="min-h-0 flex-1 overflow-auto p-4">
        <div className="mx-auto grid max-w-5xl gap-4 lg:grid-cols-[220px_minmax(0,1fr)]">
          <aside className="rounded-md border bg-card p-3 text-sm">
            <div className="font-medium">Settings</div>
            <div className="mt-3 text-muted-foreground">General</div>
            <div className="mt-2 font-medium">Models</div>
            <div className="mt-2 text-muted-foreground">Transcription</div>
            <div className="mt-2 text-muted-foreground">Storage</div>
          </aside>
          <Card className="rounded-md">
            <CardHeader>
              <CardTitle>Local speech recognition</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4 text-sm">
              {model ? (
                <ModelCard
                  model={model}
                  runtimeSelection={runtimeSelection}
                  onDownload={onDownload}
                  onVerify={onVerify}
                  onRemove={onRemove}
                />
              ) : (
                <div className="rounded-md border bg-muted/40 p-3 text-muted-foreground">
                  No transcription models are available.
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </section>
    </main>
  );
}

function ModelCard({
  model,
  runtimeSelection,
  onDownload,
  onVerify,
  onRemove,
}: {
  model: TranscriptionModelStatus;
  runtimeSelection: RuntimeSelection;
  onDownload: (modelId: string) => void;
  onVerify: (modelId: string) => void;
  onRemove: (modelId: string) => void;
}) {
  const ready = model.installStatus === "ready";
  const failed = model.installStatus === "failed";
  const missing = model.installStatus === "missing";

  return (
    <div className="rounded-md border p-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <div className="flex items-center gap-2">
            {ready ? (
              <CheckCircle2 className="h-4 w-4 text-emerald-600" />
            ) : failed ? (
              <XCircle className="h-4 w-4 text-destructive" />
            ) : (
              <Download className="h-4 w-4 text-muted-foreground" />
            )}
            <h2 className="text-base font-semibold">{model.displayName}</h2>
          </div>
          <p className="mt-1 max-w-2xl text-muted-foreground">
            Local Parakeet v3 model used for transcript timestamps before EDL and caption generation.
          </p>
        </div>
        <div className="flex gap-2">
          {missing ? (
            <Button onClick={() => onDownload(model.modelId)}>
              <Download className="mr-2 h-4 w-4" />
              Download
            </Button>
          ) : (
            <Button variant="outline" onClick={() => onVerify(model.modelId)}>
              <RefreshCw className="mr-2 h-4 w-4" />
              Verify
            </Button>
          )}
          {!missing ? (
            <Button variant="outline" onClick={() => onRemove(model.modelId)}>
              <Trash2 className="mr-2 h-4 w-4" />
              Remove
            </Button>
          ) : null}
        </div>
      </div>
      <div className="mt-4 grid gap-2 sm:grid-cols-4">
        <StatusTile label="Status" value={statusLabel(model.installStatus)} />
        <StatusTile label="Size" value={formatBytes(model.approximateSizeBytes)} />
        <StatusTile label="Runtime" value={runtimeLabel(runtimeSelection)} />
        <StatusTile label="Path" value={model.localPath} />
      </div>
    </div>
  );
}

function StatusTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md border bg-muted/30 p-3">
      <div className="text-xs font-medium text-muted-foreground">{label}</div>
      <div className="mt-1 truncate text-sm">{value}</div>
    </div>
  );
}

function statusLabel(status: TranscriptionModelStatus["installStatus"]) {
  const labels = {
    missing: "Missing",
    downloading: "Downloading",
    verifying: "Verifying",
    ready: "Ready",
    failed: "Failed",
  } satisfies Record<TranscriptionModelStatus["installStatus"], string>;
  return labels[status];
}

function runtimeLabel(runtime: RuntimeSelection) {
  const labels = {
    native: "Rust native",
    python: "Python fallback",
    unavailable: "Unavailable",
  } satisfies Record<RuntimeSelection, string>;
  return labels[runtime];
}

function formatBytes(bytes: number) {
  return `${(bytes / 1_000_000_000).toFixed(2)} GB`;
}
```

- [ ] **Step 5: Run UI tests**

Run:

```bash
rtk pnpm test -- src/components/settings/model-settings.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Commit Task 6**

Run:

```bash
rtk git add src/lib/transcription-models.ts src/components/settings/model-settings.tsx src/components/settings/model-settings.test.tsx
rtk git commit -m "feat: add model settings screen"
```

## Task 7: Wire Settings Route And Generate Edit Gating UI

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/agent-panel.tsx`
- Add or modify: `src/components/workspace/agent-panel.test.tsx`

- [ ] **Step 1: Add failing Agent panel gating test**

Create `src/components/workspace/agent-panel.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AgentPanel } from "./agent-panel";

describe("AgentPanel model gate", () => {
  it("blocks Generate edit and opens settings when local model is missing", async () => {
    const onGenerateEdit = vi.fn();
    const onOpenModelSettings = vi.fn();

    render(
      <AgentPanel
        modelReady={false}
        onOpenModelSettings={onOpenModelSettings}
        onGenerateEdit={onGenerateEdit}
      />,
    );

    expect(screen.getByText("Parakeet model required.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open model settings" }));

    expect(onOpenModelSettings).toHaveBeenCalledTimes(1);
    expect(onGenerateEdit).not.toHaveBeenCalled();
  });

  it("allows Generate edit when local model is ready", async () => {
    const onGenerateEdit = vi.fn();

    render(<AgentPanel modelReady onGenerateEdit={onGenerateEdit} />);

    await userEvent.click(screen.getByRole("button", { name: "Generate edit" }));

    expect(onGenerateEdit).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk pnpm test -- src/components/workspace/agent-panel.test.tsx
```

Expected: FAIL because `modelReady` and `onOpenModelSettings` props do not exist.

- [ ] **Step 3: Update Agent panel props and gating**

Modify `src/components/workspace/agent-panel.tsx` props:

```tsx
interface AgentPanelProps {
  mediaId?: string;
  latestRequest?: EditJobRequest | null;
  modelReady?: boolean;
  onOpenModelSettings?: () => void;
  onGenerateEdit?: (request: EditJobRequest) => void;
}
```

Update function signature:

```tsx
export function AgentPanel({
  mediaId = "media-1",
  latestRequest,
  modelReady = true,
  onOpenModelSettings,
  onGenerateEdit,
}: AgentPanelProps) {
```

Replace the Generate button area with:

```tsx
{modelReady ? (
  <Button className="w-full" onClick={generateEdit} disabled={!prompt.trim()}>
    Generate edit
  </Button>
) : (
  <div className="space-y-2 rounded-md border bg-muted/40 p-3">
    <div className="text-xs font-medium">Parakeet model required.</div>
    <p className="text-xs text-muted-foreground">
      Install the local transcription model before generating edits from source media.
    </p>
    <Button className="w-full" variant="outline" onClick={onOpenModelSettings}>
      Open model settings
    </Button>
  </div>
)}
```

- [ ] **Step 4: Update EditorWorkspace to accept settings callback and model readiness**

Modify `src/components/workspace/editor-workspace.tsx`:

```tsx
interface EditorWorkspaceProps {
  modelReady?: boolean;
  onOpenModelSettings?: () => void;
}

export function EditorWorkspace({
  modelReady = true,
  onOpenModelSettings,
}: EditorWorkspaceProps) {
```

Add a Settings button in the header button group:

```tsx
<Button variant="outline" size="sm" onClick={onOpenModelSettings}>
  Settings
</Button>
```

Pass props to `AgentPanel`:

```tsx
<AgentPanel
  latestRequest={latestEditRequest}
  modelReady={modelReady}
  onOpenModelSettings={onOpenModelSettings}
  onGenerateEdit={setLatestEditRequest}
/>
```

- [ ] **Step 5: Wire App route state and model loading**

Modify `src/App.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ModelSettings } from "@/components/settings/model-settings";
import { EditorWorkspace } from "@/components/workspace/editor-workspace";
import {
  downloadTranscriptionModel,
  getTranscriptionRuntimeStatus,
  listTranscriptionModels,
  removeTranscriptionModel,
  type RuntimeSelection,
  type TranscriptionModelStatus,
  verifyTranscriptionModel,
} from "@/lib/transcription-models";

type AppView = "editor" | "model-settings";

export default function App() {
  const [view, setView] = useState<AppView>("editor");
  const [models, setModels] = useState<TranscriptionModelStatus[]>([]);
  const [runtimeSelection, setRuntimeSelection] = useState<RuntimeSelection>("unavailable");

  async function refreshModels() {
    const [modelList, runtime] = await Promise.all([
      listTranscriptionModels(),
      getTranscriptionRuntimeStatus(),
    ]);
    setModels(modelList);
    setRuntimeSelection(runtime);
  }

  useEffect(() => {
    void refreshModels();
  }, []);

  async function runModelAction(action: () => Promise<TranscriptionModelStatus>) {
    await action();
    await refreshModels();
  }

  const modelReady = models.some((model) => model.installStatus === "ready");

  if (view === "model-settings") {
    return (
      <ModelSettings
        models={models}
        runtimeSelection={runtimeSelection}
        onBack={() => setView("editor")}
        onDownload={(modelId) =>
          void runModelAction(() => downloadTranscriptionModel(modelId))
        }
        onVerify={(modelId) =>
          void runModelAction(() => verifyTranscriptionModel(modelId))
        }
        onRemove={(modelId) =>
          void runModelAction(() => removeTranscriptionModel(modelId))
        }
      />
    );
  }

  return (
    <EditorWorkspace
      modelReady={modelReady}
      onOpenModelSettings={() => setView("model-settings")}
    />
  );
}
```

- [ ] **Step 6: Run frontend tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/agent-panel.test.tsx src/components/settings/model-settings.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Commit Task 7**

Run:

```bash
rtk git add src/App.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx
rtk git commit -m "feat: gate edit generation on local model"
```

## Task 8: Verification And Visual QA

**Files:**
- Verify: full project

- [ ] **Step 1: Run Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
rtk pnpm test
```

Expected: PASS.

- [ ] **Step 3: Run TypeScript lint**

Run:

```bash
rtk pnpm lint
```

Expected: PASS.

- [ ] **Step 4: Run full verification**

Run:

```bash
rtk pnpm run verify
```

Expected: PASS.

- [ ] **Step 5: Start dev server for visual QA**

Run:

```bash
rtk pnpm dev
```

Expected: Vite starts on `http://127.0.0.1:5173` or another available port.

- [ ] **Step 6: Inspect Settings / Models in Browser**

Open the local URL in Browser. Check:

- Settings button is visible in the editor header.
- Settings / Models screen has no overlapping text at desktop width.
- Narrow viewport keeps status tiles readable.
- Missing model state shows Download.
- Ready model state shows Verify and Remove.
- Generate edit is blocked when `modelReady` is false.

- [ ] **Step 7: Commit final verification fixes**

If verification required code changes, run:

```bash
rtk git add <changed-files>
rtk git commit -m "fix: polish parakeet model manager"
```

If no files changed, do not create an empty commit.
