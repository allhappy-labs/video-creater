# Core ML Temporal Transcription Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Temporal transcription real by using a global `.mlmodelc` Parakeet model and a replaceable Core ML-first Rust runtime path, with Python/Transformers removed from active readiness and workflow behavior.

**Architecture:** Rust owns global model state, runtime selection, Temporal activity contracts, transcript artifacts, and split-project mutation. `VideoCreaterTranscribeMediaWorkflow` runs `ProbeMedia -> RunTranscription -> StoreTranscript`; activities call a replaceable runtime interface so Core ML is the first implementation and future runtimes can replace it without changing workflow inputs. Python/Transformers code is removed from active runtime selection, UI readiness, and tests.

**Tech Stack:** Rust 1.87, Tauri 2, Temporal Rust SDK, serde, tempfile, `coreml-native = "0.2.0"` on macOS, existing split-project storage, React/Vitest only for runtime status label cleanup.

---

## File Structure

- Modify `src-tauri/Cargo.toml`: add target-specific `coreml-native = "0.2.0"` dependency for macOS.
- Modify `src-tauri/src/transcription/model.rs`: keep `.mlmodelc` Parakeet catalog as canonical and remove Transformers runtime metadata from active catalog data.
- Modify `src-tauri/src/transcription/runtime.rs`: replace probe-only/Python selection with Core ML-first runtime selection and execution traits.
- Modify `src-tauri/src/transcription/job.rs`: add workflow-safe transcription request, probe output, raw artifact, store output, and typed errors.
- Modify `src-tauri/src/edit/transcript.rs`: rename parser entry point away from Python-specific `parse_parakeet_transcript` or add a runtime-neutral artifact parser while keeping cleanup helpers.
- Modify `src-tauri/src/workflows/mod.rs`: add transcribe start request builder, real activity functions, and a real feature-gated `VideoCreaterTranscribeMediaWorkflow`.
- Modify `src-tauri/src/main.rs`: remove Python runner config state and use Core ML-only runtime readiness.
- Delete `src-tauri/python/parakeet_runner.py` after Rust imports no longer reference it.
- Modify `src-tauri/tests/transcription_models.rs`: rewrite runtime/model tests for global Core ML behavior and Python removal.
- Modify `src-tauri/tests/temporal_workflows.rs`: add activity/start-request/store tests for transcription workflow.
- Modify `src-tauri/tests/one_click_edit.rs`: update transcript parser tests to the runtime-neutral artifact parser.
- Modify `src/lib/transcription-models.ts`: keep public runtime status type stable if possible; remove Python-specific assumptions only if surfaced.
- Modify `src/App.test.tsx` and `src/components/settings/model-settings.test.tsx`: update runtime expectations from Python fallback to native/Core ML.

## Task 1: Model Catalog Stays `.mlmodelc` And Python Runtime Metadata Is Removed

**Files:**
- Modify: `src-tauri/src/transcription/model.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write the failing catalog tests**

Add these tests to `src-tauri/tests/transcription_models.rs`, replacing the existing tests that expect `TransformersPython` support:

```rust
#[test]
fn catalog_entry_declares_coreml_bundle_without_transformers_runtime() {
    let entry = parakeet_v3_catalog_entry();

    assert_eq!(entry.family, TranscriptionModelFamily::Parakeet);
    assert_eq!(entry.artifact_format, TranscriptionModelArtifactFormat::CoreMlBundle);
    assert_eq!(entry.supported_runtimes, vec![TranscriptionRuntimeId::CoreMl]);
    assert!(!entry
        .required_files
        .iter()
        .any(|file| file.path == "model.safetensors"));
}

#[test]
fn parakeet_v3_catalog_entry_lists_compiled_coreml_bundles() {
    let entry = parakeet_v3_catalog_entry();
    let required_paths = entry
        .required_files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();

    assert!(required_paths.contains(&"Encoder.mlmodelc/coremldata.bin"));
    assert!(required_paths.contains(&"Decoder.mlmodelc/coremldata.bin"));
    assert!(required_paths.contains(&"JointDecisionv3.mlmodelc/coremldata.bin"));
    assert!(required_paths.contains(&"Preprocessor.mlmodelc/coremldata.bin"));
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models catalog_entry_declares_coreml_bundle_without_transformers_runtime
```

Expected: FAIL while old tests/imports still reference `TransformersPythonRuntime` or while `TranscriptionRuntimeId::TransformersPython` remains reachable in active catalog assertions.

- [ ] **Step 3: Remove Transformers from active model metadata**

In `src-tauri/src/transcription/model.rs`, replace the runtime id enum with:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionRuntimeId {
    CoreMl,
}
```

Keep `TranscriptionModelArtifactFormat::Transformers` only if another compile path still imports it. If no code imports it after Task 2, reduce the enum to:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionModelArtifactFormat {
    CoreMlBundle,
}
```

- [ ] **Step 4: Run the catalog tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models catalog_entry_declares_coreml_bundle_without_transformers_runtime parakeet_v3_catalog_entry_lists_compiled_coreml_bundles
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/transcription/model.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "refactor: keep transcription catalog Core ML native"
```

## Task 2: Runtime Selection Is Core ML-First And Platform-Aware

**Files:**
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/tests/transcription_models.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Write failing runtime selection tests**

Replace the Python runtime tests in `src-tauri/tests/transcription_models.rs` with these tests:

```rust
#[test]
fn runtime_selection_uses_coreml_when_model_is_ready_on_macos() {
    let model = parakeet_v3_catalog_entry();
    let runtime = TestRuntime {
        capability: RuntimeCapability::Ready,
        supported: true,
    };

    let selection = select_transcription_runtime(&model, &runtime);

    assert_eq!(selection, RuntimeSelection::Native);
}

#[test]
fn runtime_selection_reports_unsupported_platform() {
    let model = parakeet_v3_catalog_entry();
    let runtime = TestRuntime {
        capability: RuntimeCapability::UnsupportedPlatform,
        supported: true,
    };

    let selection = select_transcription_runtime(&model, &runtime);

    assert_eq!(selection, RuntimeSelection::UnsupportedPlatform);
}

#[test]
fn runtime_selection_reports_unavailable_when_coreml_cannot_load() {
    let model = parakeet_v3_catalog_entry();
    let runtime = TestRuntime {
        capability: RuntimeCapability::Unavailable,
        supported: true,
    };

    let selection = select_transcription_runtime(&model, &runtime);

    assert_eq!(selection, RuntimeSelection::Unavailable);
}
```

Update `TestRuntime` in the same test file so it implements the new one-runtime trait shape:

```rust
impl TranscriptionRuntime for TestRuntime {
    fn id(&self) -> &'static str {
        "test_runtime"
    }

    fn supports(&self, _model: &TranscriptionModelCatalogEntry) -> bool {
        self.supported
    }

    fn probe(&self, _model: &TranscriptionModelCatalogEntry) -> RuntimeCapability {
        self.capability.clone()
    }
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models runtime_selection_
```

Expected: FAIL because `RuntimeCapability::UnsupportedPlatform` and `RuntimeSelection::UnsupportedPlatform` do not exist and `select_transcription_runtime` still accepts native plus Python.

- [ ] **Step 3: Add the Core ML dependency for macOS only**

In `src-tauri/Cargo.toml`, add:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
coreml-native = "0.2.0"
```

- [ ] **Step 4: Replace runtime selection types**

In `src-tauri/src/transcription/runtime.rs`, replace the capability/selection enums and selector with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapability {
    Ready,
    UnsupportedPlatform,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSelection {
    Native,
    UnsupportedPlatform,
    Unavailable,
}

pub trait TranscriptionRuntime {
    fn id(&self) -> &'static str;
    fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool;
    fn probe(&self, model: &TranscriptionModelCatalogEntry) -> RuntimeCapability;
}

pub fn select_transcription_runtime<R>(
    model: &TranscriptionModelCatalogEntry,
    runtime: &R,
) -> RuntimeSelection
where
    R: TranscriptionRuntime,
{
    if !runtime.supports(model) {
        return RuntimeSelection::Unavailable;
    }

    match runtime.probe(model) {
        RuntimeCapability::Ready => RuntimeSelection::Native,
        RuntimeCapability::UnsupportedPlatform => RuntimeSelection::UnsupportedPlatform,
        RuntimeCapability::Unavailable => RuntimeSelection::Unavailable,
    }
}
```

- [ ] **Step 5: Replace `CoreMlParakeetRuntime::probe`**

In the `CoreMlParakeetRuntime` implementation in `runtime.rs`, use cfg-specific platform behavior:

```rust
impl TranscriptionRuntime for CoreMlParakeetRuntime {
    fn id(&self) -> &'static str {
        "core_ml_parakeet"
    }

    fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool {
        model.artifact_format == TranscriptionModelArtifactFormat::CoreMlBundle
    }

    fn probe(&self, model: &TranscriptionModelCatalogEntry) -> RuntimeCapability {
        if !self.supports(model) {
            return RuntimeCapability::Unavailable;
        }

        #[cfg(target_os = "macos")]
        {
            RuntimeCapability::Ready
        }

        #[cfg(not(target_os = "macos"))]
        {
            RuntimeCapability::UnsupportedPlatform
        }
    }
}
```

Keep model-file presence checks in `TranscriptionModelStore`; runtime probe decides platform/runtime capability after store verification.

- [ ] **Step 6: Delete Python runtime types**

Remove `TransformersPythonRuntime` and `PythonParakeetRuntime` from `runtime.rs`. Remove their imports and tests from `src-tauri/tests/transcription_models.rs`.

- [ ] **Step 7: Run tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models runtime_selection_
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/Cargo.toml src-tauri/src/transcription/runtime.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "refactor: select native Core ML transcription runtime"
```

## Task 3: Remove Python Runtime Wiring From Tauri Commands

**Files:**
- Modify: `src-tauri/src/main.rs`
- Delete: `src-tauri/python/parakeet_runner.py`
- Modify: `src-tauri/tests/transcription_models.rs`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Write failing Rust test for runtime command behavior**

Add a unit test in `src-tauri/src/main.rs` test module:

```rust
#[test]
fn runtime_selection_does_not_require_python_runner_path() {
    let entry = video_creater_lib::transcription::model::parakeet_v3_catalog_entry();
    let runtime = video_creater_lib::transcription::runtime::CoreMlParakeetRuntime;
    let selection =
        video_creater_lib::transcription::runtime::select_transcription_runtime(&entry, &runtime);

    #[cfg(target_os = "macos")]
    assert_eq!(
        selection,
        video_creater_lib::transcription::runtime::RuntimeSelection::Native
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        selection,
        video_creater_lib::transcription::runtime::RuntimeSelection::UnsupportedPlatform
    );
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml runtime_selection_does_not_require_python_runner_path
```

Expected: FAIL while `main.rs` still imports `TransformersPythonRuntime` or requires `TranscriptionRuntimeConfigState`.

- [ ] **Step 3: Remove Python config state from `main.rs`**

In `src-tauri/src/main.rs`:

Remove this import:

```rust
TransformersPythonRuntime,
```

Remove this struct:

```rust
struct TranscriptionRuntimeConfigState {
    runner_script_path: PathBuf,
}
```

Change `generate_one_click_edit_for_project` signature from:

```rust
runtime_config: tauri::State<'_, TranscriptionRuntimeConfigState>,
```

to no runtime config argument, and call:

```rust
validate_generate_edit_transcription_gate(&project, &request, model_state)?;
```

Change `get_transcription_runtime_status` to:

```rust
fn get_transcription_runtime_status(
    model_id: Option<String>,
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<RuntimeSelection, String> {
    current_transcription_runtime_selection(model_id.as_deref(), model_state)
}
```

Change `validate_generate_edit_transcription_gate` and `current_transcription_runtime_selection` to remove `runtime_config`.

Inside `current_transcription_runtime_selection`, replace native plus Python construction with:

```rust
let native = CoreMlParakeetRuntime;
Ok(select_transcription_runtime(&entry, &native))
```

Remove `resolve_parakeet_runner_path`.

In `run()`, remove `runner_script_path` resolution and remove:

```rust
app.manage(TranscriptionRuntimeConfigState { runner_script_path });
```

- [ ] **Step 4: Delete the Python runner**

Delete:

```text
src-tauri/python/parakeet_runner.py
```

- [ ] **Step 5: Update frontend bridge test**

In `src/App.test.tsx`, keep the existing expectation that `get_transcription_runtime_status` is invoked with `{ modelId: null }`. Do not add Python-specific labels or setup strings.

- [ ] **Step 6: Run tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml runtime_selection_does_not_require_python_runner_path
rtk pnpm test src/App.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add src-tauri/src/main.rs src-tauri/python/parakeet_runner.py src/App.test.tsx
rtk git commit -m "refactor: remove Python transcription runtime wiring"
```

## Task 4: Add Runtime-Neutral Transcript Artifact Parsing

**Files:**
- Modify: `src-tauri/src/edit/transcript.rs`
- Modify: `src-tauri/tests/one_click_edit.rs`

- [ ] **Step 1: Write failing parser test**

In `src-tauri/tests/one_click_edit.rs`, replace `parses_parakeet_words_from_runner_json` with:

```rust
#[test]
fn parses_runtime_transcript_artifact_words() {
    let json = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": "media-1",
        "engine": "core_ml_parakeet",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "runtimeId": "core_ml_parakeet",
        "languageMode": "en",
        "tokens": [
            {"token": "Strong", "start": 1.0, "end": 1.4, "confidence": null},
            {"token": " hand", "start": 1.4, "end": 1.8, "confidence": null},
            {"token": ".", "start": 1.8, "end": 1.8, "confidence": null}
        ]
    });

    let transcript = parse_transcript_artifact(&json).expect("parse transcript artifact");

    assert_eq!(transcript.id, "transcript-media-1");
    assert_eq!(transcript.media_id, "media-1");
    assert_eq!(transcript.engine.as_deref(), Some("core_ml_parakeet"));
    assert_eq!(transcript.words.len(), 3);
    assert_eq!(transcript.words[0].text, "Strong");
    assert_eq!(transcript.words[1].text, "hand");
    assert_eq!(transcript.segments[0].text, "Strong hand.");
}
```

Update imports:

```rust
use video_creater_lib::edit::transcript::{
    apply_transcript_replacements, clean_transcript_text, group_words_into_caption_segments,
    parse_transcript_artifact, TranscriptReplacement,
};
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test one_click_edit parses_runtime_transcript_artifact_words
```

Expected: FAIL because `parse_transcript_artifact` does not exist.

- [ ] **Step 3: Add runtime-neutral parser types**

In `src-tauri/src/edit/transcript.rs`, add:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptArtifactToken {
    token: String,
    start: f64,
    end: f64,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptArtifact {
    schema_version: u32,
    media_id: String,
    engine: Option<String>,
    tokens: Vec<TranscriptArtifactToken>,
}
```

Add:

```rust
pub fn parse_transcript_artifact(
    value: &serde_json::Value,
) -> Result<Transcript, TranscriptParseError> {
    let output: TranscriptArtifact =
        serde_json::from_value(value.clone()).map_err(TranscriptParseError::InvalidShape)?;
    if output.schema_version != 1 {
        return Err(TranscriptParseError::UnsupportedSchemaVersion(output.schema_version));
    }
    if output.tokens.is_empty() {
        return Err(TranscriptParseError::MissingTokens);
    }

    let words = output
        .tokens
        .into_iter()
        .filter_map(|token| {
            let text = clean_transcript_text(&token.token);
            if text.is_empty() {
                return None;
            }

            Some(TranscriptWord {
                text,
                start_seconds: token.start,
                end_seconds: token.end,
                confidence: token.confidence,
                speaker: None,
            })
        })
        .collect::<Vec<_>>();

    let segments = group_words_into_caption_segments(&words);

    Ok(Transcript {
        id: format!("transcript-{}", output.media_id),
        media_id: output.media_id,
        engine: output.engine,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments,
        words,
    })
}
```

Add this enum variant:

```rust
#[error("unsupported transcript artifact schema version: {0}")]
UnsupportedSchemaVersion(u32),
```

- [ ] **Step 4: Keep old parser as a thin compatibility wrapper**

Replace the body of `parse_parakeet_transcript` with a conversion into schema version 1 so existing callers keep compiling during the transition:

```rust
pub fn parse_parakeet_transcript(
    media_id: &str,
    value: &serde_json::Value,
) -> Result<Transcript, TranscriptParseError> {
    let output: ParakeetOutput =
        serde_json::from_value(value.clone()).map_err(TranscriptParseError::InvalidShape)?;
    let tokens = output.tokens.ok_or(TranscriptParseError::MissingTokens)?;
    let artifact = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": media_id,
        "engine": output.model,
        "tokens": tokens.into_iter().map(|token| serde_json::json!({
            "token": token.token,
            "start": token.start,
            "end": token.end,
            "confidence": null
        })).collect::<Vec<_>>()
    });

    parse_transcript_artifact(&artifact)
}
```

- [ ] **Step 5: Run parser tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test one_click_edit parses_runtime_transcript_artifact_words groups_transcript_words_into_caption_segments
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/edit/transcript.rs src-tauri/tests/one_click_edit.rs
rtk git commit -m "feat: parse native transcription artifacts"
```

## Task 5: Add Temporal Transcription Activity Contracts And Start Request

**Files:**
- Modify: `src-tauri/src/transcription/job.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/tests/temporal_workflows.rs`
- Modify: `src/lib/project.ts`
- Modify: `src/lib/project.test.ts`

- [ ] **Step 1: Write failing Rust start-request test**

Add to `src-tauri/tests/temporal_workflows.rs`:

```rust
#[test]
fn temporal_transcribe_media_start_request_contains_replayable_input() {
    let request = video_creater_lib::workflows::temporal_transcribe_media_start_request(
        "project-1",
        "/tmp/video-project",
        "media-1",
        "transcribe-job-1",
        "en",
    );

    assert_eq!(request.workflow_type, "VideoCreaterTranscribeMediaWorkflow");
    assert_eq!(
        request.workflow_id,
        "video-creater/project-1/transcribe-media/transcribe-job-1"
    );
    assert_eq!(request.input["projectId"], "project-1");
    assert_eq!(request.input["projectDir"], "/tmp/video-project");
    assert_eq!(request.input["mediaId"], "media-1");
    assert_eq!(request.input["jobId"], "transcribe-job-1");
    assert_eq!(request.input["languageMode"], "en");
    assert_eq!(
        request.activity_types,
        vec!["ProbeMedia", "RunTranscription", "StoreTranscript"]
    );
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows temporal_transcribe_media_start_request_contains_replayable_input
```

Expected: FAIL because `temporal_transcribe_media_start_request` does not exist.

- [ ] **Step 3: Add transcription job types**

In `src-tauri/src/transcription/job.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalTranscribeMediaWorkflowInput {
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub language_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalTranscribeProbeOutput {
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub language_mode: String,
    pub source_path: String,
    pub model_id: String,
    pub model_path: String,
    pub runtime_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalTranscribeRunOutput {
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub language_mode: String,
    pub model_id: String,
    pub runtime_id: String,
    pub artifact_path: String,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalStoreTranscriptOutput {
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub transcript_id: String,
    pub segment_count: usize,
    pub word_count: usize,
}
```

Add imports at the top of `job.rs`:

```rust
use serde::{Deserialize, Serialize};
```

- [ ] **Step 4: Add start-request builder**

In `src-tauri/src/workflows/mod.rs`, add:

```rust
pub fn temporal_transcribe_media_start_request(
    project_id: &str,
    project_dir: &str,
    media_id: &str,
    job_id: &str,
    language_mode: &str,
) -> TemporalWorkflowStartRequest {
    temporal_workflow_start_request(
        TemporalWorkflowKind::TranscribeMedia,
        project_id,
        job_id,
        json!({
            "projectId": project_id,
            "projectDir": project_dir,
            "mediaId": media_id,
            "jobId": job_id,
            "languageMode": language_mode,
        }),
    )
}
```

- [ ] **Step 5: Add frontend bridge function**

In `src/lib/project.ts`, add:

```ts
export async function buildTemporalTranscribeMediaStartRequest(input: {
  projectId: string;
  projectDir: string;
  mediaId: string;
  jobId: string;
  languageMode: string;
}): Promise<TemporalWorkflowStartRequest> {
  return invoke("build_temporal_transcribe_media_start_request", input);
}
```

In `src/lib/project.test.ts`, add an invoke-contract test next to the other Temporal start request tests:

```ts
it("calls the Rust Temporal transcribe media start request command", async () => {
  const request = await buildTemporalTranscribeMediaStartRequest({
    projectId: "project-1",
    projectDir: "/tmp/project",
    mediaId: "media-1",
    jobId: "transcribe-1",
    languageMode: "en",
  });

  expect(invoke).toHaveBeenCalledWith("build_temporal_transcribe_media_start_request", {
    projectId: "project-1",
    projectDir: "/tmp/project",
    mediaId: "media-1",
    jobId: "transcribe-1",
    languageMode: "en",
  });
  expect(request.workflowType).toBe("VideoCreaterTranscribeMediaWorkflow");
});
```

- [ ] **Step 6: Run tests and verify GREEN for Rust, RED for frontend until command exists**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows temporal_transcribe_media_start_request_contains_replayable_input
rtk pnpm test src/lib/project.test.ts
```

Expected: Rust PASS. Frontend FAIL if the TypeScript import is not added to the test file; add the import and rerun until PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add src-tauri/src/transcription/job.rs src-tauri/src/workflows/mod.rs src-tauri/tests/temporal_workflows.rs src/lib/project.ts src/lib/project.test.ts
rtk git commit -m "feat: add Temporal transcribe media contract"
```

## Task 6: Implement `ProbeMedia` Activity

**Files:**
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Write failing activity test**

Add to `src-tauri/tests/temporal_workflows.rs`:

```rust
#[test]
fn transcribe_probe_activity_resolves_media_and_global_model() {
    let root = tempfile::tempdir().expect("temp project");
    let model_root = tempfile::tempdir().expect("temp model root");
    let mut project = video_creater_lib::project::fixtures::sample_project();
    project.media[0].id = "media-1".to_string();
    project.media[0].path = "/tmp/source.mov".to_string();
    video_creater_lib::project::split::save_split_project(root.path(), &project)
        .expect("save split project");
    let store = video_creater_lib::transcription::store::TranscriptionModelStore::new(
        model_root.path().to_path_buf(),
    );
    let entry = video_creater_lib::transcription::model::parakeet_v3_catalog_entry();
    let model_dir = model_root
        .path()
        .join(video_creater_lib::transcription::model::safe_model_dir_name(entry.id));
    std::fs::create_dir_all(&model_dir).expect("model dir");
    write_required_files(&entry, &model_dir);
    store.verify(entry.id).expect("verify model");

    let output = video_creater_lib::workflows::temporal_transcribe_probe_media_activity_value_with_store(
        serde_json::json!({
            "projectId": project.id,
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en"
        }),
        &store,
    )
    .expect("probe output");

    assert_eq!(output["status"], "ready");
    assert_eq!(output["mediaId"], "media-1");
    assert_eq!(output["modelId"], entry.id);
    assert_eq!(output["runtimeId"], "core_ml_parakeet");
}
```

Use the existing `write_required_files` helper from `transcription_models.rs`; if it is private, copy the helper into `temporal_workflows.rs`:

```rust
fn write_required_files(
    entry: &video_creater_lib::transcription::model::TranscriptionModelCatalogEntry,
    model_dir: &std::path::Path,
) {
    for file in &entry.required_files {
        let path = model_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("model file parent");
        }
        std::fs::write(path, b"present").expect("write model file");
    }
}
```

- [ ] **Step 2: Run test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows transcribe_probe_activity_resolves_media_and_global_model
```

Expected: FAIL because `temporal_transcribe_probe_media_activity_value_with_store` does not exist.

- [ ] **Step 3: Add activity helper**

In `src-tauri/src/workflows/mod.rs`, add:

```rust
pub fn temporal_transcribe_probe_media_activity_value_with_store(
    input: Value,
    store: &TranscriptionModelStore,
) -> Result<Value, TemporalWorkflowInputError> {
    let input: TemporalTranscribeMediaWorkflowInput =
        serde_json::from_value(input).map_err(|error| {
            TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
        })?;
    let project = load_split_project(Path::new(&input.project_dir))
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    let media = project
        .media
        .iter()
        .find(|media| media.id == input.media_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("mediaId".to_string()))?;
    if media.path.trim().is_empty() {
        return Err(TemporalWorkflowInputError::BlankField("media.path".to_string()));
    }
    let status = store.active_status().map_err(|error| {
        TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
    })?;
    if status.install_status != ModelInstallStatus::Ready {
        return Err(TemporalWorkflowInputError::DecodeActivityInput(
            "transcription model is not ready".to_string(),
        ));
    }
    let entry = transcription_model_catalog_entry(&status.model_id)
        .ok_or_else(|| TemporalWorkflowInputError::MissingField("modelId".to_string()))?;
    let runtime = CoreMlParakeetRuntime;
    let selection = select_transcription_runtime(&entry, &runtime);
    if selection != RuntimeSelection::Native {
        return Err(TemporalWorkflowInputError::DecodeActivityInput(format!(
            "transcription runtime is {selection:?}"
        )));
    }
    let model_path = store
        .model_dir(entry.id)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;

    serde_json::to_value(TemporalTranscribeProbeOutput {
        project_id: input.project_id,
        project_dir: input.project_dir,
        media_id: input.media_id,
        job_id: input.job_id,
        language_mode: input.language_mode,
        source_path: media.path.clone(),
        model_id: entry.id.to_string(),
        model_path: model_path.display().to_string(),
        runtime_id: runtime.id().to_string(),
    })
    .map(|mut value| {
        value
            .as_object_mut()
            .expect("probe output object")
            .insert("status".to_string(), json!("ready"));
        value
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}
```

Add required imports in `workflows/mod.rs`:

```rust
use crate::transcription::job::{
    TemporalTranscribeMediaWorkflowInput, TemporalTranscribeProbeOutput,
};
use crate::transcription::model::{transcription_model_catalog_entry, ModelInstallStatus};
use crate::transcription::runtime::{
    select_transcription_runtime, CoreMlParakeetRuntime, RuntimeSelection, TranscriptionRuntime,
};
use crate::transcription::store::TranscriptionModelStore;
```

- [ ] **Step 4: Add non-store convenience helper**

Add:

```rust
pub fn temporal_transcribe_probe_media_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let input_for_root: TemporalTranscribeMediaWorkflowInput =
        serde_json::from_value(input.clone()).map_err(|error| {
            TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
        })?;
    let store = TranscriptionModelStore::new(default_global_transcription_model_root()?);
    temporal_transcribe_probe_media_activity_value_with_store(input, &store)
}
```

If `default_global_transcription_model_root` does not exist yet, add it in `store.rs`:

```rust
pub fn default_global_transcription_model_root() -> Result<PathBuf, ModelStoreError> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| ModelStoreError::Download("HOME is not set".to_string()))?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join("video-creater")
        .join("models"))
}
```

Use this only for worker-process defaults; Tauri app commands continue to use `app.path().app_data_dir()?.join("models")`.

- [ ] **Step 5: Wire activity method**

In the feature-gated `VideoCreaterTemporalActivities::probe_media`, replace placeholder registration with:

```rust
super::temporal_transcribe_probe_media_activity_value(input).map_err(activity_error)
```

- [ ] **Step 6: Run test and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows transcribe_probe_activity_resolves_media_and_global_model
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add src-tauri/src/workflows/mod.rs src-tauri/src/transcription/store.rs src-tauri/tests/temporal_workflows.rs
rtk git commit -m "feat: probe media for Temporal transcription"
```

## Task 7: Implement Core ML Runtime Execution Boundary And `RunTranscription`

**Files:**
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/src/transcription/job.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Write failing run activity test with injectable backend**

Add to `src-tauri/tests/temporal_workflows.rs`:

```rust
#[test]
fn run_transcription_activity_writes_runtime_artifact() {
    let root = tempfile::tempdir().expect("temp project");
    let artifact_dir = root.path().join("workflow-artifacts");

    let output = video_creater_lib::workflows::temporal_transcribe_run_activity_value_with_backend(
        serde_json::json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "sourcePath": "/tmp/source.mov",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "modelPath": "/tmp/model",
            "runtimeId": "core_ml_parakeet"
        }),
        &video_creater_lib::transcription::runtime::FixtureTranscriptionBackend {
            tokens: vec![
                video_creater_lib::transcription::runtime::TranscriptToken {
                    token: "Hello".to_string(),
                    start: 0.1,
                    end: 0.4,
                    confidence: None,
                },
            ],
        },
        &artifact_dir,
    )
    .expect("run output");

    assert_eq!(output["tokenCount"], 1);
    let artifact_path = output["artifactPath"].as_str().expect("artifact path");
    let artifact: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(artifact_path).expect("read artifact"))
            .expect("artifact json");
    assert_eq!(artifact["schemaVersion"], 1);
    assert_eq!(artifact["runtimeId"], "core_ml_parakeet");
    assert_eq!(artifact["tokens"][0]["token"], "Hello");
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows run_transcription_activity_writes_runtime_artifact
```

Expected: FAIL because the backend and activity helper do not exist.

- [ ] **Step 3: Add execution structs and backend trait**

In `src-tauri/src/transcription/runtime.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptToken {
    pub token: String,
    pub start: f64,
    pub end: f64,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRuntimeJob {
    pub media_id: String,
    pub source_path: String,
    pub model_path: String,
    pub language_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRuntimeOutput {
    pub runtime_id: String,
    pub model_id: String,
    pub tokens: Vec<TranscriptToken>,
}

pub trait TranscriptionBackend {
    fn transcribe(
        &self,
        job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError>;
}

#[derive(Debug, Error)]
pub enum TranscriptionRuntimeError {
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("Core ML inference failed: {0}")]
    Inference(String),
}

#[derive(Debug, Clone)]
pub struct FixtureTranscriptionBackend {
    pub tokens: Vec<TranscriptToken>,
}

impl TranscriptionBackend for FixtureTranscriptionBackend {
    fn transcribe(
        &self,
        _job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        Ok(TranscriptionRuntimeOutput {
            runtime_id: "core_ml_parakeet".to_string(),
            model_id: model_id.to_string(),
            tokens: self.tokens.clone(),
        })
    }
}
```

- [ ] **Step 4: Add macOS Core ML backend shell**

In `runtime.rs`, add:

```rust
#[derive(Debug, Clone, Copy)]
pub struct CoreMlParakeetBackend;

impl TranscriptionBackend for CoreMlParakeetBackend {
    fn transcribe(
        &self,
        job: &TranscriptionRuntimeJob,
        model_id: &str,
    ) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = job;
            let _ = model_id;
            Err(TranscriptionRuntimeError::UnsupportedPlatform)
        }

        #[cfg(target_os = "macos")]
        {
            run_coreml_parakeet(job, model_id)
        }
    }
}
```

Add the first macOS implementation with model loading validation and a clear inference error. Task 8 must replace this guarded error with real token output before the feature can be considered complete:

```rust
#[cfg(target_os = "macos")]
fn run_coreml_parakeet(
    job: &TranscriptionRuntimeJob,
    model_id: &str,
) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
    let model_root = std::path::Path::new(&job.model_path);
    for bundle in [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
        "JointDecisionv3.mlmodelc",
    ] {
        let bundle_path = model_root.join(bundle);
        if !bundle_path.is_dir() {
            return Err(TranscriptionRuntimeError::Inference(format!(
                "missing Core ML bundle {}",
                bundle_path.display()
            )));
        }
    }

    Err(TranscriptionRuntimeError::Inference(
        "Core ML Parakeet decode loop is not producing tokens yet".to_string(),
    ))
}
```

This keeps production honest: the workflow cannot report success until tokens exist. The fixture backend makes activity/storage tests deterministic while Task 8 handles production Core ML inference.

- [ ] **Step 5: Add run activity helper**

In `src-tauri/src/workflows/mod.rs`, add:

```rust
pub fn temporal_transcribe_run_activity_value_with_backend<B: TranscriptionBackend>(
    input: Value,
    backend: &B,
    artifact_dir: &Path,
) -> Result<Value, TemporalWorkflowInputError> {
    let probe: TemporalTranscribeProbeOutput =
        serde_json::from_value(input).map_err(|error| {
            TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
        })?;
    std::fs::create_dir_all(artifact_dir).map_err(|error| {
        TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
    })?;
    let job = TranscriptionRuntimeJob {
        media_id: probe.media_id.clone(),
        source_path: probe.source_path.clone(),
        model_path: probe.model_path.clone(),
        language_mode: probe.language_mode.clone(),
    };
    let output = backend
        .transcribe(&job, &probe.model_id)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
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
    std::fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&artifact).map_err(|error| {
            TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
        })?,
    )
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;

    serde_json::to_value(TemporalTranscribeRunOutput {
        project_id: probe.project_id,
        project_dir: probe.project_dir,
        media_id: job.media_id,
        job_id: probe.job_id,
        language_mode: job.language_mode,
        model_id: output.model_id,
        runtime_id: output.runtime_id,
        artifact_path: artifact_path.display().to_string(),
        token_count: artifact["tokens"].as_array().map(|tokens| tokens.len()).unwrap_or(0),
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}
```

Add:

```rust
pub fn temporal_transcribe_run_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let probe: TemporalTranscribeProbeOutput =
        serde_json::from_value(input.clone()).map_err(|error| {
            TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
        })?;
    let artifact_dir = Path::new(&probe.project_dir)
        .join("workflow-artifacts")
        .join(&probe.job_id);
    temporal_transcribe_run_activity_value_with_backend(
        input,
        &CoreMlParakeetBackend,
        &artifact_dir,
    )
}
```

- [ ] **Step 6: Wire Temporal activity method**

In feature-gated `VideoCreaterTemporalActivities::run_transcription`, replace placeholder registration with:

```rust
super::temporal_transcribe_run_activity_value(input).map_err(activity_error)
```

- [ ] **Step 7: Run activity test and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows run_transcription_activity_writes_runtime_artifact
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/src/transcription/runtime.rs src-tauri/src/transcription/job.rs src-tauri/src/workflows/mod.rs src-tauri/tests/temporal_workflows.rs
rtk git commit -m "feat: run Temporal transcription through native backend"
```

## Task 8: Replace Core ML Guard With Real Parakeet Token Output

**Files:**
- Modify: `src-tauri/src/transcription/runtime.rs`
- Create: `src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Add a Core ML inspection binary target**

In `src-tauri/Cargo.toml`, add:

```toml
[[bin]]
name = "video-creater-inspect-coreml-parakeet"
path = "src/bin/video-creater-inspect-coreml-parakeet.rs"
```

Create `src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs`:

```rust
#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use coreml_native::{ComputeUnits, Model};
    use std::path::PathBuf;

    let model_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: video-creater-inspect-coreml-parakeet <model-root>"))?;
    for bundle in [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
        "JointDecisionv3.mlmodelc",
    ] {
        let path = model_root.join(bundle);
        let model = Model::load(&path, ComputeUnits::All)?;
        println!("MODEL {}", path.display());
        println!("INPUTS");
        for input in model.inputs() {
            println!(
                "  {} {:?} shape={:?} optional={}",
                input.name(),
                input.feature_type(),
                input.shape(),
                input.is_optional(),
            );
        }
        println!("OUTPUTS");
        for output in model.outputs() {
            println!(
                "  {} {:?} shape={:?}",
                output.name(),
                output.feature_type(),
                output.shape(),
            );
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "Core ML Parakeet inspection requires macOS"
    ))
}
```

- [ ] **Step 2: Run the inspector against the installed global model**

Run:

```bash
rtk cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-inspect-coreml-parakeet -- "$HOME/Library/Application Support/video-creater/models/nvidia__parakeet-tdt-0.6b-v3"
```

Expected on macOS with the model installed: output lists inputs and outputs for `Preprocessor.mlmodelc`, `Encoder.mlmodelc`, `Decoder.mlmodelc`, and `JointDecisionv3.mlmodelc`. Expected on non-macOS: fails with `Core ML Parakeet inspection requires macOS`.

- [ ] **Step 3: Write failing Core ML guard-removal test**

Add this macOS-only test to `src-tauri/tests/transcription_models.rs`:

```rust
#[cfg(target_os = "macos")]
#[test]
fn coreml_backend_returns_tokens_for_short_wav_fixture_when_model_is_installed() {
    let model_root = std::env::var("VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT")
        .expect("set VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT to a verified .mlmodelc Parakeet model root");
    let source_path = std::env::var("VIDEO_CREATER_TEST_TRANSCRIPTION_WAV")
        .expect("set VIDEO_CREATER_TEST_TRANSCRIPTION_WAV to a short 16 kHz mono wav fixture");
    let backend = video_creater_lib::transcription::runtime::CoreMlParakeetBackend;
    let output = backend
        .transcribe(
            &video_creater_lib::transcription::runtime::TranscriptionRuntimeJob {
                media_id: "media-1".to_string(),
                source_path,
                model_path: model_root,
                language_mode: "en".to_string(),
            },
            "nvidia/parakeet-tdt-0.6b-v3",
        )
        .expect("Core ML Parakeet output");

    assert_eq!(output.runtime_id, "core_ml_parakeet");
    assert!(!output.tokens.is_empty());
    assert!(output.tokens.iter().all(|token| token.end >= token.start));
}
```

This test is intentionally environment-gated because the model is a large global artifact and should not be committed to the repository.

- [ ] **Step 4: Run the test and verify RED on a configured macOS machine**

Run:

```bash
VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT="$HOME/Library/Application Support/video-creater/models/nvidia__parakeet-tdt-0.6b-v3" \
VIDEO_CREATER_TEST_TRANSCRIPTION_WAV="/absolute/path/to/short-16khz-mono.wav" \
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models coreml_backend_returns_tokens_for_short_wav_fixture_when_model_is_installed
```

Expected: FAIL with `Core ML Parakeet decode loop is not producing tokens yet`.

- [ ] **Step 5: Implement the production decode loop**

In `src-tauri/src/transcription/runtime.rs`, replace the guarded error in `run_coreml_parakeet` with a model-specific pipeline using `coreml_native::{BorrowedTensor, ComputeUnits, Model}`:

```rust
#[cfg(target_os = "macos")]
fn run_coreml_parakeet(
    job: &TranscriptionRuntimeJob,
    model_id: &str,
) -> Result<TranscriptionRuntimeOutput, TranscriptionRuntimeError> {
    let model_root = std::path::Path::new(&job.model_path);
    let preprocessor = coreml_native::Model::load(
        model_root.join("Preprocessor.mlmodelc"),
        coreml_native::ComputeUnits::All,
    )
    .map_err(|error| TranscriptionRuntimeError::Inference(error.to_string()))?;
    let encoder = coreml_native::Model::load(
        model_root.join("Encoder.mlmodelc"),
        coreml_native::ComputeUnits::All,
    )
    .map_err(|error| TranscriptionRuntimeError::Inference(error.to_string()))?;
    let decoder = coreml_native::Model::load(
        model_root.join("Decoder.mlmodelc"),
        coreml_native::ComputeUnits::All,
    )
    .map_err(|error| TranscriptionRuntimeError::Inference(error.to_string()))?;
    let joint = coreml_native::Model::load(
        model_root.join("JointDecisionv3.mlmodelc"),
        coreml_native::ComputeUnits::All,
    )
    .map_err(|error| TranscriptionRuntimeError::Inference(error.to_string()))?;

    let audio = read_transcription_wav_mono_16khz(&job.source_path)?;
    let preprocessor_output = run_parakeet_preprocessor(&preprocessor, &audio)?;
    let encoder_output = run_parakeet_encoder(&encoder, preprocessor_output)?;
    let tokens = run_parakeet_decoder_joint_loop(
        &decoder,
        &joint,
        encoder_output,
        model_root,
    )?;
    if tokens.is_empty() {
        return Err(TranscriptionRuntimeError::Inference(
            "Core ML Parakeet emitted no tokens".to_string(),
        ));
    }

    Ok(TranscriptionRuntimeOutput {
        runtime_id: "core_ml_parakeet".to_string(),
        model_id: model_id.to_string(),
        tokens,
    })
}
```

Add helper functions in the same file. Use the input/output names printed by `video-creater-inspect-coreml-parakeet`; keep those names as constants near the helpers:

```rust
#[cfg(target_os = "macos")]
fn read_transcription_wav_mono_16khz(
    source_path: &str,
) -> Result<Vec<f32>, TranscriptionRuntimeError> {
    let bytes = std::fs::read(source_path)
        .map_err(|error| TranscriptionRuntimeError::Inference(error.to_string()))?;
    decode_pcm_wav_mono_16khz(&bytes)
}
```

If the app has no WAV decoder yet, implement `decode_pcm_wav_mono_16khz` for PCM s16le fixtures first and return a typed `Inference` error for other WAV encodings:

```rust
#[cfg(target_os = "macos")]
fn decode_pcm_wav_mono_16khz(bytes: &[u8]) -> Result<Vec<f32>, TranscriptionRuntimeError> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(TranscriptionRuntimeError::Inference(
            "expected RIFF/WAVE input".to_string(),
        ));
    }
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
    let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let bits_per_sample = u16::from_le_bytes([bytes[34], bytes[35]]);
    if channels != 1 || sample_rate != 16_000 || bits_per_sample != 16 {
        return Err(TranscriptionRuntimeError::Inference(format!(
            "expected mono 16 kHz 16-bit PCM wav, got channels={channels} sampleRate={sample_rate} bits={bits_per_sample}"
        )));
    }
    let data_start = bytes
        .windows(4)
        .position(|window| window == b"data")
        .map(|index| index + 8)
        .ok_or_else(|| TranscriptionRuntimeError::Inference("missing wav data chunk".to_string()))?;
    let mut samples = Vec::new();
    for chunk in bytes[data_start..].chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(sample as f32 / i16::MAX as f32);
    }
    Ok(samples)
}
```

Implement `run_parakeet_preprocessor`, `run_parakeet_encoder`, and `run_parakeet_decoder_joint_loop` using `BorrowedTensor::from_f32`, `Model::predict`, and `Prediction::get_f32` as shown in the `coreml-native` examples. The exact feature names must match the inspector output from Step 2.

- [ ] **Step 6: Run the configured Core ML test and verify GREEN**

Run the same command from Step 4.

Expected: PASS on a configured macOS machine. If the model input/output names differ from assumed constants, update only those constants and rerun.

- [ ] **Step 7: Commit**

```bash
rtk git add src-tauri/Cargo.toml src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs src-tauri/src/transcription/runtime.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: execute Parakeet with Core ML runtime"
```

## Task 9: Implement `StoreTranscript` Activity

**Files:**
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Write failing store activity test**

Add to `src-tauri/tests/temporal_workflows.rs`:

```rust
#[test]
fn store_transcript_activity_replaces_project_transcript() {
    let root = tempfile::tempdir().expect("temp project");
    let mut project = video_creater_lib::project::fixtures::sample_project();
    project.id = "project-1".to_string();
    project.transcripts.push(video_creater_lib::project::model::Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("old".to_string()),
        raw_artifact_path: None,
        repairs: vec![],
        segments: vec![],
        words: vec![],
    });
    video_creater_lib::project::split::save_split_project(root.path(), &project)
        .expect("save project");
    let artifact_path = root.path().join("artifact.json");
    std::fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "mediaId": "media-1",
            "engine": "core_ml_parakeet",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": "core_ml_parakeet",
            "languageMode": "en",
            "tokens": [
                {"token": "Hello", "start": 0.1, "end": 0.4, "confidence": null}
            ]
        }))
        .expect("artifact json"),
    )
    .expect("write artifact");

    let output = video_creater_lib::workflows::temporal_transcribe_store_activity_value(
        serde_json::json!({
            "projectId": "project-1",
            "projectDir": root.path().display().to_string(),
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en",
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": "core_ml_parakeet",
            "artifactPath": artifact_path.display().to_string(),
            "tokenCount": 1
        }),
    )
    .expect("store output");

    assert_eq!(output["transcriptId"], "transcript-media-1");
    assert_eq!(output["wordCount"], 1);
    let stored = video_creater_lib::project::split::load_split_project(root.path())
        .expect("load stored project");
    assert_eq!(stored.transcripts.len(), 1);
    assert_eq!(stored.transcripts[0].engine.as_deref(), Some("core_ml_parakeet"));
    assert_eq!(stored.transcripts[0].raw_artifact_path.as_deref(), Some(artifact_path.to_str().unwrap()));
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows store_transcript_activity_replaces_project_transcript
```

Expected: FAIL because `temporal_transcribe_store_activity_value` does not exist.

- [ ] **Step 3: Implement store activity helper**

In `src-tauri/src/workflows/mod.rs`, add:

```rust
pub fn temporal_transcribe_store_activity_value(
    input: Value,
) -> Result<Value, TemporalWorkflowInputError> {
    let run: TemporalTranscribeRunOutput =
        serde_json::from_value(input).map_err(|error| {
            TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
        })?;
    let artifact_text = std::fs::read_to_string(&run.artifact_path).map_err(|error| {
        TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
    })?;
    let artifact_json: Value = serde_json::from_str(&artifact_text).map_err(|error| {
        TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
    })?;
    let mut transcript = parse_transcript_artifact(&artifact_json)
        .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))?;
    transcript.raw_artifact_path = Some(run.artifact_path.clone());
    let project_dir = Path::new(&run.project_dir);
    let mut project = load_split_project(project_dir)
        .map_err(|error| TemporalWorkflowInputError::LoadSplitProject(error.to_string()))?;
    project
        .transcripts
        .retain(|existing| existing.media_id != transcript.media_id);
    let transcript_id = transcript.id.clone();
    let segment_count = transcript.segments.len();
    let word_count = transcript.words.len();
    project.transcripts.push(transcript);
    save_split_project(project_dir, &project)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))?;

    serde_json::to_value(TemporalStoreTranscriptOutput {
        project_id: run.project_id,
        project_dir: run.project_dir,
        media_id: run.media_id,
        job_id: run.job_id,
        transcript_id,
        segment_count,
        word_count,
    })
    .map_err(|error| TemporalWorkflowInputError::DecodeActivityInput(error.to_string()))
}
```

Add imports:

```rust
use crate::edit::transcript::parse_transcript_artifact;
use crate::transcription::job::{TemporalStoreTranscriptOutput, TemporalTranscribeRunOutput};
```

- [ ] **Step 4: Wire Temporal activity method**

In feature-gated `VideoCreaterTemporalActivities::store_transcript`, replace placeholder registration with:

```rust
super::temporal_transcribe_store_activity_value(input).map_err(activity_error)
```

- [ ] **Step 5: Run store test and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows store_transcript_activity_replaces_project_transcript
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/workflows/mod.rs src-tauri/tests/temporal_workflows.rs
rtk git commit -m "feat: store Temporal transcript artifacts"
```

## Task 10: Replace Registered Transcription Workflow Placeholder

**Files:**
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Write failing workflow plan test**

Add to `src-tauri/tests/temporal_workflows.rs`:

```rust
#[test]
fn transcribe_workflow_activity_plan_is_not_placeholder() {
    let plan = video_creater_lib::workflows::temporal_transcribe_workflow_activity_plan_value(
        serde_json::json!({
            "projectId": "project-1",
            "projectDir": "/tmp/project",
            "mediaId": "media-1",
            "jobId": "transcribe-1",
            "languageMode": "en"
        }),
        "2026-06-28T00:00:00Z",
        Some("run-1"),
    )
    .expect("plan");

    assert_eq!(plan["status"], "planned");
    assert_eq!(plan["workflowType"], "VideoCreaterTranscribeMediaWorkflow");
    assert_eq!(plan["activityTypes"], serde_json::json!(["ProbeMedia", "RunTranscription", "StoreTranscript"]));
    assert_eq!(plan["probeMediaInput"]["mediaId"], "media-1");
    assert_eq!(plan["runTranscriptionInputFrom"], "ProbeMedia");
    assert_eq!(plan["storeTranscriptInputFrom"], "RunTranscription");
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows transcribe_workflow_activity_plan_is_not_placeholder
```

Expected: FAIL because the plan helper does not exist and the workflow macro still returns placeholder output.

- [ ] **Step 3: Add plan helper**

In `src-tauri/src/workflows/mod.rs`, add:

```rust
pub fn temporal_transcribe_workflow_activity_plan_value(
    input: Value,
    updated_at: &str,
    run_id: Option<&str>,
) -> Result<Value, TemporalWorkflowInputError> {
    let decoded: TemporalTranscribeMediaWorkflowInput =
        serde_json::from_value(input.clone()).map_err(|error| {
            TemporalWorkflowInputError::DecodeActivityInput(error.to_string())
        })?;
    Ok(json!({
        "status": "planned",
        "workflowType": "VideoCreaterTranscribeMediaWorkflow",
        "activityTypes": TemporalWorkflowKind::TranscribeMedia.activity_types(),
        "probeMediaInput": {
            "projectId": decoded.project_id,
            "projectDir": decoded.project_dir,
            "mediaId": decoded.media_id,
            "jobId": decoded.job_id,
            "languageMode": decoded.language_mode,
        },
        "runTranscriptionInputFrom": "ProbeMedia",
        "storeTranscriptInputFrom": "RunTranscription",
        "updatedAt": updated_at,
        "runId": run_id,
    }))
}
```

- [ ] **Step 4: Replace registered workflow macro use**

In the feature-gated `workflows` module, remove:

```rust
registered_workflow!(
    VideoCreaterTranscribeMediaWorkflow,
    "VideoCreaterTranscribeMediaWorkflow"
);
```

Replace it with:

```rust
#[workflow]
#[derive(Default)]
pub struct VideoCreaterTranscribeMediaWorkflow;

#[workflow_methods]
impl VideoCreaterTranscribeMediaWorkflow {
    #[run(name = "VideoCreaterTranscribeMediaWorkflow")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, input: Value) -> WorkflowResult<Value> {
        let updated_at = workflow_start_time_rfc3339(
            ctx.workflow_initial_info()
                .start_time
                .and_then(|timestamp| timestamp.try_into().ok()),
        );
        let plan = match super::temporal_transcribe_workflow_activity_plan_value(
            input.clone(),
            &updated_at,
            Some(ctx.run_id()),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return Ok(json!({
                    "status": "failed",
                    "workflowType": "VideoCreaterTranscribeMediaWorkflow",
                    "error": error.to_string(),
                    "input": input,
                }));
            }
        };
        let probe_output = ctx
            .start_activity(
                VideoCreaterTemporalActivities::probe_media,
                plan["probeMediaInput"].clone(),
                ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
            )
            .await?;
        let run_output = ctx
            .start_activity(
                VideoCreaterTemporalActivities::run_transcription,
                probe_output,
                ActivityOptions::start_to_close_timeout(Duration::from_secs(3600)),
            )
            .await?;
        let store_output = ctx
            .start_activity(
                VideoCreaterTemporalActivities::store_transcript,
                run_output,
                ActivityOptions::start_to_close_timeout(Duration::from_secs(60)),
            )
            .await?;

        Ok(json!({
            "status": "completed",
            "workflowType": "VideoCreaterTranscribeMediaWorkflow",
            "plan": plan,
            "store": store_output,
        }))
    }
}
```

- [ ] **Step 5: Run workflow tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows transcribe_workflow_activity_plan_is_not_placeholder temporal_worker_manifest_lists_registered_workflows_and_activities
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/workflows/mod.rs src-tauri/tests/temporal_workflows.rs
rtk git commit -m "feat: run real Temporal transcription workflow"
```

## Task 11: UI And TypeScript Bridge Cleanup

**Files:**
- Modify: `src/lib/transcription-models.ts`
- Modify: `src/components/settings/model-settings.tsx`
- Modify: `src/components/settings/model-settings.test.tsx`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Write failing UI test for native runtime copy**

In `src/components/settings/model-settings.test.tsx`, add or update a ready-state test:

```tsx
it("describes native Core ML runtime readiness without Python setup copy", () => {
  render(
    <ModelSettings
      models={[
        {
          modelId: "nvidia/parakeet-tdt-0.6b-v3",
          displayName: "Parakeet TDT 0.6B v3",
          isActive: true,
          installStatus: "ready",
          localPath: "/Users/me/Library/Application Support/video-creater/models/nvidia__parakeet-tdt-0.6b-v3",
          approximateSizeBytes: 485000000,
          downloadedFiles: 18,
          totalFiles: 18,
          verifiedAt: "2026-06-28T00:00:00Z",
        },
      ]}
      activeModelId="nvidia/parakeet-tdt-0.6b-v3"
      runtimeSelection="native"
      onDownload={vi.fn()}
      onCancel={vi.fn()}
      onVerify={vi.fn()}
      onRemove={vi.fn()}
      onSetActive={vi.fn()}
    />,
  );

  expect(screen.getByText(/Core ML runtime ready/i)).toBeInTheDocument();
  expect(screen.queryByText(/Python/i)).not.toBeInTheDocument();
  expect(screen.queryByText(/Transformers/i)).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run UI test and verify RED**

Run:

```bash
rtk pnpm test src/components/settings/model-settings.test.tsx
```

Expected: FAIL if current copy still mentions generic or Python fallback runtime state.

- [ ] **Step 3: Update runtime status type and labels**

In `src/lib/transcription-models.ts`, change runtime selection type to:

```ts
export type RuntimeSelection = "native" | "unsupported_platform" | "unavailable";
```

In `src/components/settings/model-settings.tsx`, map status labels as:

```ts
const runtimeLabel =
  runtimeSelection === "native"
    ? "Core ML runtime ready"
    : runtimeSelection === "unsupported_platform"
      ? "Core ML requires macOS"
      : "Core ML runtime unavailable";
```

Render `runtimeLabel` in the existing runtime status location. Do not add Python or Transformers text.

- [ ] **Step 4: Run UI tests and verify GREEN**

Run:

```bash
rtk pnpm test src/components/settings/model-settings.test.tsx src/App.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/lib/transcription-models.ts src/components/settings/model-settings.tsx src/components/settings/model-settings.test.tsx src/App.test.tsx
rtk git commit -m "fix: show Core ML transcription runtime status"
```

## Task 12: Final Verification

**Files:**
- No source edits unless verification exposes a defect.

- [ ] **Step 1: Run Rust transcription and Temporal tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models
rtk cargo test --manifest-path src-tauri/Cargo.toml --test one_click_edit
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows
```

Expected: PASS.

- [ ] **Step 2: Run worker compile check**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker
```

Expected: PASS.

- [ ] **Step 3: Run frontend tests touched by runtime copy**

Run:

```bash
rtk pnpm test src/lib/project.test.ts src/App.test.tsx src/components/settings/model-settings.test.tsx
```

Expected: PASS.

- [ ] **Step 4: Search for active Python/Transformers references**

Run:

```bash
rtk rg -n "TransformersPython|PythonParakeet|parakeet_runner|Transformers|python transcription|Python fallback" src-tauri/src src-tauri/tests src src/components
```

Expected: no matches in active app/runtime/test code. Matches in historical docs are acceptable only outside the searched paths.

- [ ] **Step 5: Inspect git status**

Run:

```bash
rtk git status --short
```

Expected: only intentional files changed, plus the pre-existing unrelated `?? palmier-pro/` directory if it is still present.

- [ ] **Step 6: Commit verification fixes if any were needed**

If Step 1 through Step 4 exposed defects and source files were changed to fix them, commit those fixes:

```bash
rtk git add <changed-files>
rtk git commit -m "fix: harden Core ML Temporal transcription"
```

If no files changed, do not create an empty commit.
