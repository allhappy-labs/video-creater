# Agent Transcript Access And Core ML Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give agents bounded access to transcript word timings and make Parakeet Core ML bundles importable, inspectable, and ready for the native decode loop.

**Architecture:** Rust remains the authority for project state, transcript reads, model storage, validation, and Temporal activities. Agents get a read-only `video_creater.transcript_words` local tool for exact transcript ranges; Core ML models are imported into the app-global model store, verified, and inspected into a JSON contract consumed by the native backend. The actual Parakeet decode loop stays behind `TranscriptionBackend` and is completed only when a compatible `.mlmodelc` bundle and WAV fixture are available.

**Tech Stack:** Rust, Tauri commands, serde JSON schemas, existing Codex local tools, `coreml-native`, Vitest/React bridge tests, existing Temporal/transcription Rust tests.

---

## File Structure

- `src-tauri/src/codex/tools.rs`: add transcript query args, local tool descriptor, call dispatch, payload builder, and validation.
- `src-tauri/tests/codex_app_server.rs`: cover tool manifest, full transcript payload, time range filtering, pagination, missing transcript/media, and invalid ranges.
- `src-tauri/src/transcription/store.rs`: add import/copy/verify support and inspection report file constants.
- `src-tauri/src/transcription/model.rs`: expose required Core ML bundle/file checks through existing catalog metadata.
- `src-tauri/src/main.rs`: add `import_transcription_model` Tauri command.
- `src/lib/transcription-models.ts`: add typed `importTranscriptionModel(modelId, sourcePath)` bridge.
- `src/App.tsx` and `src/components/settings/model-settings.tsx`: wire an import action only if a UI affordance is added in this pass; otherwise keep bridge-only.
- `src/components/settings/model-settings.test.tsx` and `src/App.test.tsx`: update only if UI props change.
- `src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs`: add JSON output mode.
- `src-tauri/tests/transcription_models.rs`: cover import verification, inspection report behavior, and env-gated Core ML decode readiness.
- `src-tauri/src/transcription/runtime.rs`: add inspection-report loading/type validation and isolate decode-loop stubs so bundle/spec failures are precise.

---

## Task 1: Add Agent Transcript Range Tool

**Files:**
- Modify: `src-tauri/src/codex/tools.rs`
- Modify: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Write failing manifest test**

Add to `src-tauri/tests/codex_app_server.rs` near the local tool manifest tests:

```rust
#[test]
fn mcp_parity_tool_manifest_exposes_transcript_words_query() {
    let tools = list_codex_local_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.name == "video_creater.transcript_words")
        .expect("transcript words tool");

    assert_eq!(tool.category, "transcription");
    assert!(tool.description.contains("word"));
    assert_eq!(
        tool.input_schema["required"],
        serde_json::json!(["mediaId"])
    );
    assert_eq!(
        tool.input_schema["properties"]["limit"]["maximum"],
        serde_json::json!(500)
    );
}
```

- [ ] **Step 2: Run manifest test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server mcp_parity_tool_manifest_exposes_transcript_words_query -- --nocapture
```

Expected: FAIL because `video_creater.transcript_words` is not in the tool list.

- [ ] **Step 3: Add tool descriptor and args**

In `src-tauri/src/codex/tools.rs`, add constants near existing tool limits:

```rust
const DEFAULT_TRANSCRIPT_WORD_LIMIT: usize = 200;
const MAX_TRANSCRIPT_WORD_LIMIT: usize = 500;
```

Add args struct near `TranscriptionReadinessArgs`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptWordsArgs {
    media_id: String,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    offset: Option<usize>,
    limit: Option<usize>,
}
```

Add descriptor in `list_codex_local_tools()` immediately after `video_creater.transcription_readiness`:

```rust
tool_descriptor(
    "video_creater.transcript_words",
    "Transcript Words",
    "transcription",
    "Return bounded transcript words with word indices and source-media timings.",
    json!({
        "type": "object",
        "required": ["mediaId"],
        "additionalProperties": false,
        "properties": {
            "mediaId": { "type": "string", "minLength": 1 },
            "startSeconds": { "type": "number", "minimum": 0 },
            "endSeconds": { "type": "number", "exclusiveMinimum": 0 },
            "offset": { "type": "integer", "minimum": 0 },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_TRANSCRIPT_WORD_LIMIT }
        }
    }),
),
```

Add dispatch in `call_codex_local_tool()`:

```rust
"video_creater.transcript_words" => {
    let args: TranscriptWordsArgs = decode_args(args)?;
    (transcript_words_payload(project, &args)?, false)
}
```

Add helper:

```rust
fn transcript_words_payload(
    project: &VideoProject,
    args: &TranscriptWordsArgs,
) -> Result<Value, CodexLocalToolError> {
    if let (Some(start), Some(end)) = (args.start_seconds, args.end_seconds) {
        if end <= start {
            return Err(CodexLocalToolError::InvalidArguments(
                "endSeconds must be greater than startSeconds".to_string(),
            ));
        }
    }
    if matches!(args.start_seconds, Some(value) if value < 0.0)
        || matches!(args.end_seconds, Some(value) if value <= 0.0)
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "transcript word range must use non-negative seconds".to_string(),
        ));
    }

    let media_exists = project.media.iter().any(|media| media.id == args.media_id);
    if !media_exists {
        return Ok(json!({
            "mediaId": args.media_id,
            "transcriptId": Value::Null,
            "error": format!("media was not found: {}", args.media_id),
            "words": [],
            "totalWords": 0,
            "matchedWords": 0,
            "returnedWords": 0,
            "nextOffset": Value::Null,
            "truncated": false
        }));
    }

    let Some(transcript) = project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == args.media_id)
    else {
        return Ok(json!({
            "mediaId": args.media_id,
            "transcriptId": Value::Null,
            "error": format!("transcript was not found for media: {}", args.media_id),
            "words": [],
            "totalWords": 0,
            "matchedWords": 0,
            "returnedWords": 0,
            "nextOffset": Value::Null,
            "truncated": false
        }));
    };

    let offset = args.offset.unwrap_or(0);
    let limit = args
        .limit
        .unwrap_or(DEFAULT_TRANSCRIPT_WORD_LIMIT)
        .min(MAX_TRANSCRIPT_WORD_LIMIT);
    let start = args.start_seconds.unwrap_or(0.0);
    let end = args.end_seconds.unwrap_or(f64::INFINITY);
    let matched = transcript
        .words
        .iter()
        .enumerate()
        .filter(|(_, word)| word.end_seconds >= start && word.start_seconds <= end)
        .collect::<Vec<_>>();
    let words = matched
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(word_index, word)| {
            json!({
                "wordIndex": word_index,
                "text": word.text,
                "startSeconds": word.start_seconds,
                "endSeconds": word.end_seconds,
                "confidence": word.confidence,
                "speaker": word.speaker
            })
        })
        .collect::<Vec<_>>();
    let next_offset = offset + words.len();
    let truncated = next_offset < matched.len();

    Ok(json!({
        "mediaId": args.media_id,
        "transcriptId": transcript.id,
        "engine": transcript.engine,
        "rawArtifactPath": transcript.raw_artifact_path,
        "range": {
            "startSeconds": args.start_seconds,
            "endSeconds": args.end_seconds
        },
        "offset": offset,
        "limit": limit,
        "totalWords": transcript.words.len(),
        "matchedWords": matched.len(),
        "returnedWords": words.len(),
        "nextOffset": if truncated { json!(next_offset) } else { Value::Null },
        "truncated": truncated,
        "words": words
    }))
}
```

- [ ] **Step 4: Run manifest test and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server mcp_parity_tool_manifest_exposes_transcript_words_query -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Add payload tests**

Add tests to `src-tauri/tests/codex_app_server.rs` near `transcription_readiness` tests:

```rust
#[test]
fn transcript_words_tool_returns_word_indices_and_timings() {
    let mut project = sample_project();
    project.transcripts[0].words = vec![
        TranscriptWord {
            text: "Hook".to_string(),
            start_seconds: 0.4,
            end_seconds: 0.8,
            confidence: Some(0.91),
            speaker: Some("A".to_string()),
        },
        TranscriptWord {
            text: "moment".to_string(),
            start_seconds: 1.0,
            end_seconds: 1.4,
            confidence: None,
            speaker: None,
        },
    ];

    let result = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({ "mediaId": "media-1" }),
    )
    .expect("transcript words");

    assert!(!result.mutates_project);
    assert_eq!(result.payload["transcriptId"], json!("transcript-1"));
    assert_eq!(result.payload["totalWords"], json!(2));
    assert_eq!(result.payload["words"][0]["wordIndex"], json!(0));
    assert_eq!(result.payload["words"][0]["startSeconds"], json!(0.4));
    assert_eq!(result.payload["words"][0]["endSeconds"], json!(0.8));
    assert_eq!(result.payload["words"][0]["speaker"], json!("A"));
}

#[test]
fn transcript_words_tool_filters_ranges_and_paginates() {
    let mut project = sample_project();
    project.transcripts[0].words = (0..6)
        .map(|index| TranscriptWord {
            text: format!("word-{index}"),
            start_seconds: index as f64,
            end_seconds: index as f64 + 0.5,
            confidence: None,
            speaker: None,
        })
        .collect();

    let result = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": "media-1",
            "startSeconds": 1.2,
            "endSeconds": 4.2,
            "offset": 1,
            "limit": 2
        }),
    )
    .expect("transcript words");

    assert_eq!(result.payload["matchedWords"], json!(4));
    assert_eq!(result.payload["returnedWords"], json!(2));
    assert_eq!(result.payload["nextOffset"], json!(3));
    assert_eq!(result.payload["truncated"], json!(true));
    assert_eq!(result.payload["words"][0]["wordIndex"], json!(2));
    assert_eq!(result.payload["words"][1]["wordIndex"], json!(3));
}

#[test]
fn transcript_words_tool_rejects_invalid_range() {
    let project = sample_project();
    let error = call_codex_local_tool(
        &project,
        "video_creater.transcript_words",
        json!({
            "mediaId": "media-1",
            "startSeconds": 4.0,
            "endSeconds": 4.0
        }),
    )
    .expect_err("invalid range");

    assert!(error.to_string().contains("endSeconds"));
}
```

- [ ] **Step 6: Run payload tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server transcript_words_tool -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit transcript tool**

Run:

```bash
rtk git add src-tauri/src/codex/tools.rs src-tauri/tests/codex_app_server.rs
rtk git commit -m "feat: expose transcript word timings to agents"
```

Expected: commit succeeds.

---

## Task 2: Add Core ML Model Import To Store And Tauri Bridge

**Files:**
- Modify: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/transcription_models.rs`
- Modify: `src/lib/transcription-models.ts`

- [ ] **Step 1: Write failing store import tests**

Add to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn store_import_rejects_missing_coreml_bundle_file() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());

    fs::create_dir_all(source.path().join("Encoder.mlmodelc")).expect("encoder");

    let error = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect_err("missing required files");

    assert!(error.to_string().contains("Preprocessor.mlmodelc"));
}

#[test]
fn store_import_copies_coreml_bundle_and_verifies_manifest() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let model_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    assert!(model_dir.join("Preprocessor.mlmodelc/coremldata.bin").is_file());
    assert!(model_dir.join(MODEL_MANIFEST_FILE_NAME).is_file());
}
```

Add helper near other test helpers:

```rust
fn create_fake_coreml_parakeet_tree(root: &Path) {
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
        let file_path = root.join(path);
        fs::create_dir_all(file_path.parent().expect("parent")).expect("parent");
        fs::write(&file_path, b"fake").expect("write fake model file");
    }
}
```

- [ ] **Step 2: Run import tests and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_import_ -- --nocapture
```

Expected: FAIL because `import_model` does not exist.

- [ ] **Step 3: Implement `TranscriptionModelStore::import_model`**

In `src-tauri/src/transcription/store.rs`, add error variants:

```rust
#[error("failed to create model import directory {path}: {source}")]
CreateImportDir { path: String, source: std::io::Error },
#[error("failed to copy model file from {source_path} to {target_path}: {source}")]
CopyImportFile {
    source_path: String,
    target_path: String,
    source: std::io::Error,
},
#[error("failed to replace imported model directory {path}: {source}")]
ReplaceImportDir { path: String, source: std::io::Error },
#[error("missing imported model file {0}")]
MissingImportFile(String),
```

Add method on `TranscriptionModelStore`:

```rust
pub fn import_model(
    &self,
    model_id: &str,
    source_dir: &Path,
) -> Result<TranscriptionModelStatus, ModelStoreError> {
    let entry = self.catalog_entry(model_id)?;
    if !source_dir.is_dir() {
        return Err(ModelStoreError::MissingImportFile(
            source_dir.display().to_string(),
        ));
    }
    for required in &entry.required_files {
        let source_path = source_dir.join(&required.path);
        if !path_exists(&source_path)? || !source_path.is_file() {
            return Err(ModelStoreError::MissingImportFile(required.path.clone()));
        }
    }

    let model_dir = self.model_dir_for_entry(&entry);
    if source_dir == model_dir {
        return self.verify(entry.id);
    }

    let import_dir = model_dir.with_extension("importing");
    if path_exists(&import_dir)? {
        fs::remove_dir_all(&import_dir).map_err(|source| ModelStoreError::ReplaceImportDir {
            path: import_dir.display().to_string(),
            source,
        })?;
    }
    fs::create_dir_all(&import_dir).map_err(|source| ModelStoreError::CreateImportDir {
        path: import_dir.display().to_string(),
        source,
    })?;

    for required in &entry.required_files {
        let source_path = source_dir.join(&required.path);
        let target_path = import_dir.join(&required.path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| ModelStoreError::CreateImportDir {
                path: parent.display().to_string(),
                source,
            })?;
        }
        fs::copy(&source_path, &target_path).map_err(|source| ModelStoreError::CopyImportFile {
            source_path: source_path.display().to_string(),
            target_path: target_path.display().to_string(),
            source,
        })?;
    }

    if path_exists(&model_dir)? {
        fs::remove_dir_all(&model_dir).map_err(|source| ModelStoreError::ReplaceImportDir {
            path: model_dir.display().to_string(),
            source,
        })?;
    }
    fs::rename(&import_dir, &model_dir).map_err(|source| ModelStoreError::ReplaceImportDir {
        path: model_dir.display().to_string(),
        source,
    })?;

    self.verify(entry.id)
}
```

- [ ] **Step 4: Run store tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_import_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Add Tauri command and bridge**

In `src-tauri/src/main.rs`, add command near `download_transcription_model`:

```rust
#[tauri::command]
async fn import_transcription_model(
    model_id: String,
    source_path: String,
    state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<TranscriptionModelStatus, String> {
    let store = {
        let store = state
            .0
            .lock()
            .map_err(|_| "model store lock failed".to_string())?;
        store.clone()
    };

    tauri::async_runtime::spawn_blocking(move || {
        store
            .import_model(&model_id, Path::new(&source_path))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("model import task failed: {error}"))?
}
```

Add it to `tauri::generate_handler!`.

In `src/lib/transcription-models.ts`, add:

```ts
export function importTranscriptionModel(
  modelId: string,
  sourcePath: string,
): Promise<TranscriptionModelStatus> {
  return invoke("import_transcription_model", { modelId, sourcePath });
}
```

- [ ] **Step 6: Run compile checks**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater
rtk pnpm test src/App.test.tsx src/components/settings/model-settings.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Commit import bridge**

Run:

```bash
rtk git add src-tauri/src/transcription/store.rs src-tauri/src/main.rs src-tauri/tests/transcription_models.rs src/lib/transcription-models.ts
rtk git commit -m "feat: import Core ML transcription models"
```

Expected: commit succeeds.

---

## Task 3: Add JSON Core ML Inspection Report

**Files:**
- Modify: `src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing inspector JSON test**

Add a non-macOS-safe test that validates CLI argument behavior by compiling the bin, and a macOS env-gated test for real model inspection:

```rust
#[test]
fn coreml_inspector_json_contract_is_documented_for_installed_model() {
    let Some(model_root) = std::env::var_os("VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT") else {
        eprintln!("skipping Core ML inspector JSON test: VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT is not set");
        return;
    };

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_video-creater-inspect-coreml-parakeet"))
            .arg(model_root)
            .arg("--json")
            .output()
            .expect("run inspector");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let json: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("inspector json");
        assert_eq!(json["schemaVersion"], serde_json::json!(1));
        assert_eq!(json["runtimeId"], serde_json::json!("core_ml_parakeet"));
        assert_eq!(json["bundles"].as_array().expect("bundles").len(), 4);
    }
}
```

- [ ] **Step 2: Run inspector test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models coreml_inspector_json_contract_is_documented_for_installed_model -- --nocapture
```

Expected: PASS in skip mode when env var is not set; FAIL when env var is set because `--json` is not supported yet.

- [ ] **Step 3: Implement `--json` output**

In `src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs`, define serializable structs:

```rust
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectionReport {
    schema_version: u32,
    model_id: String,
    runtime_id: String,
    bundles: Vec<BundleReport>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleReport {
    name: String,
    path: String,
    inputs: Vec<FeatureReport>,
    outputs: Vec<FeatureReport>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureReport {
    name: String,
    feature_type: String,
    shape: Option<Vec<i64>>,
    data_type: Option<String>,
    optional: Option<bool>,
}
```

Parse args:

```rust
let args = std::env::args().skip(1).collect::<Vec<_>>();
let model_root = args
    .first()
    .map(PathBuf::from)
    .ok_or_else(|| anyhow::anyhow!("usage: video-creater-inspect-coreml-parakeet <model-root> [--json]"))?;
let json_mode = args.iter().any(|arg| arg == "--json");
```

Build `InspectionReport` while loading the same four bundles. For JSON mode, print:

```rust
println!("{}", serde_json::to_string_pretty(&report)?);
```

For text mode, keep existing output format.

- [ ] **Step 4: Run inspector compile/test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models coreml_inspector_json_contract_is_documented_for_installed_model -- --nocapture
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-inspect-coreml-parakeet
```

Expected: PASS in skip mode when no model root is set; compile succeeds.

- [ ] **Step 5: Commit inspector JSON**

Run:

```bash
rtk git add src-tauri/src/bin/video-creater-inspect-coreml-parakeet.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: inspect Core ML Parakeet bundles as JSON"
```

Expected: commit succeeds.

---

## Task 4: Save Inspection Report During Import

**Files:**
- Modify: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [ ] **Step 1: Write failing saved-report test**

Add to `src-tauri/tests/transcription_models.rs`:

```rust
#[test]
fn store_import_writes_coreml_inspection_summary_contract() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());

    store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    let report_path = root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .join(COREML_INSPECTION_FILE_NAME);
    assert!(report_path.is_file());
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report_path).expect("read report"))
            .expect("report json");
    assert_eq!(report["schemaVersion"], json!(1));
    assert_eq!(report["runtimeId"], json!("core_ml_parakeet"));
}
```

- [ ] **Step 2: Run report test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_import_writes_coreml_inspection_summary_contract -- --nocapture
```

Expected: FAIL because no report is written.

- [ ] **Step 3: Write inspection summary report after verification**

In `src-tauri/src/transcription/store.rs`, add:

```rust
pub const COREML_INSPECTION_FILE_NAME: &str = "video-creater-coreml-inspection.json";
```

After `self.verify(entry.id)?` in `import_model`, write an inspection summary report for import verification tests:

```rust
let status = self.verify(entry.id)?;
let report_path = self
    .model_dir_for_entry(&entry)
    .join(COREML_INSPECTION_FILE_NAME);
let report = serde_json::json!({
    "schemaVersion": 1,
    "modelId": entry.id,
    "runtimeId": "core_ml_parakeet",
    "bundles": [
        {"name": "Preprocessor.mlmodelc"},
        {"name": "Encoder.mlmodelc"},
        {"name": "Decoder.mlmodelc"},
        {"name": "JointDecisionv3.mlmodelc"}
    ]
});
fs::write(
    &report_path,
    serde_json::to_string_pretty(&report).map_err(ModelStoreError::Serialize)?,
)
.map_err(|source| ModelStoreError::Write {
    path: report_path.display().to_string(),
    source,
})?;
Ok(status)
```

Keep the full Core ML IO report in the CLI inspector; this store report is the import-side contract marker until the CLI report can be injected from a real macOS bundle.

- [ ] **Step 4: Run report test and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models store_import_writes_coreml_inspection_summary_contract -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit saved report**

Run:

```bash
rtk git add src-tauri/src/transcription/store.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: record Core ML inspection report on import"
```

Expected: commit succeeds.

---

## Task 5: Harden Runtime Errors Around Inspection Contract

**Files:**
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/tests/temporal_workflows.rs`

- [ ] **Step 1: Update failing runtime test expectation**

In `src-tauri/tests/temporal_workflows.rs`, replace `default_coreml_run_activity_reports_decode_loop_not_ready` expectation with a missing-inspection-report check:

```rust
#[test]
fn default_coreml_run_activity_requires_coreml_inspection_report() {
    let root = tempfile::tempdir().expect("model root");
    for bundle in [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
        "JointDecisionv3.mlmodelc",
    ] {
        fs::create_dir_all(root.path().join(bundle)).expect("bundle dir");
    }
    let media = tempfile::NamedTempFile::new().expect("media");
    let error = temporal_transcribe_run_activity_value(json!({
        "projectId": "project-1",
        "projectDir": root.path().display().to_string(),
        "mediaId": "media-1",
        "mediaPath": media.path().display().to_string(),
        "jobId": "transcribe-1",
        "durationSeconds": 1.0,
        "languageMode": "en",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "modelPath": root.path().display().to_string(),
        "runtimeId": "core_ml_parakeet"
    }))
    .expect_err("missing inspection report should fail before decoding");

    assert!(error.to_string().contains("inspection report"));
}
```

- [ ] **Step 2: Run test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows default_coreml_run_activity_requires_coreml_inspection_report -- --nocapture
```

Expected: FAIL because the runtime still reports the generic decode-loop message.

- [ ] **Step 3: Add inspection report validation to backend**

In `src-tauri/src/transcription/runtime.rs`, add:

```rust
const COREML_INSPECTION_FILE_NAME: &str = "video-creater-coreml-inspection.json";
```

In `run_coreml_parakeet`, after bundle directory checks:

```rust
let inspection_path = model_root.join(COREML_INSPECTION_FILE_NAME);
if !inspection_path.is_file() {
    return Err(TranscriptionRuntimeError::Inference(format!(
        "missing Core ML inspection report {}",
        inspection_path.display()
    )));
}
let inspection_json = std::fs::read_to_string(&inspection_path).map_err(|error| {
    TranscriptionRuntimeError::Inference(format!(
        "failed to read Core ML inspection report {}: {error}",
        inspection_path.display()
    ))
})?;
let inspection: serde_json::Value = serde_json::from_str(&inspection_json).map_err(|error| {
    TranscriptionRuntimeError::Inference(format!(
        "failed to parse Core ML inspection report {}: {error}",
        inspection_path.display()
    ))
})?;
if inspection["runtimeId"] != "core_ml_parakeet" {
    return Err(TranscriptionRuntimeError::Inference(
        "Core ML inspection report runtimeId mismatch".to_string(),
    ));
}
```

Keep the final error:

```rust
Err(TranscriptionRuntimeError::Inference(
    "Core ML Parakeet decode loop is awaiting model IO wiring".to_string(),
))
```

- [ ] **Step 4: Run runtime tests and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows default_coreml_run_activity_requires_coreml_inspection_report default_coreml_run_activity_requires_compiled_model_bundles -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit runtime contract errors**

Run:

```bash
rtk git add src-tauri/src/transcription/runtime.rs src-tauri/tests/temporal_workflows.rs
rtk git commit -m "fix: require Core ML inspection report before decoding"
```

Expected: commit succeeds.

---

## Task 6: Final Verification

**Files:**
- No source edits unless verification exposes a defect.

- [ ] **Step 1: Run Codex app-server tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server
```

Expected: PASS.

- [ ] **Step 2: Run transcription model tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models
```

Expected: PASS.

- [ ] **Step 3: Run Temporal workflow tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows
```

Expected: PASS.

- [ ] **Step 4: Run worker and inspector compile checks**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker
rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-inspect-coreml-parakeet
```

Expected: PASS.

- [ ] **Step 5: Run frontend touched tests**

Run:

```bash
rtk pnpm test src/App.test.tsx src/components/settings/model-settings.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Search for retired runtime references**

Run:

```bash
rtk rg -n "TransformersPython|PythonParakeet|parakeet_runner|Transformers|python transcription|Python fallback" src-tauri/src src-tauri/tests src src/components
```

Expected: no matches.

- [ ] **Step 7: Inspect git status**

Run:

```bash
rtk git status --short --branch
```

Expected: only unrelated pre-existing untracked files, if any.
