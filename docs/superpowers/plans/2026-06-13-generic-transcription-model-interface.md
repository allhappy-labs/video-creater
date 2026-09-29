# Generic Transcription Model Interface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make transcription model management generic so Parakeet is one catalog entry instead of the shape of the app.

**Architecture:** Add model-family, artifact-format, runtime-id, active-model, and model-aware runtime support at the Rust boundary. Keep existing Tauri commands compatible while adding active-model commands and generic UI copy. Parakeet-specific names remain only in catalog data, tests for the Parakeet adapter, and concrete runtime implementation names.

**Tech Stack:** Rust, Tauri commands, serde, React, TypeScript, Vitest, Testing Library.

---

## File Structure

- Modify `src-tauri/src/transcription/model.rs`: add catalog metadata enums and generic catalog helpers.
- Modify `src-tauri/src/transcription/runtime.rs`: make runtime support model-aware and rename generic runtime implementation types.
- Modify `src-tauri/src/transcription/store.rs`: list catalog entries, track active model id, expose active model operations.
- Modify `src-tauri/src/main.rs`: wire active-model commands and model-aware runtime status.
- Modify `src-tauri/tests/transcription_models.rs`: tests for generic catalog, active model, runtime support, and compatibility.
- Modify `src/lib/transcription-models.ts`: add active model fields and commands.
- Modify `src/App.tsx`: use active model status for editor gating.
- Modify `src/components/workspace/agent-panel.tsx`: use generic copy.
- Modify `src/components/workspace/editor-workspace.tsx`: pass generic readiness prop names.
- Modify `src/components/workspace/agent-panel.test.tsx`: assert generic blocked copy.
- Modify `src/components/settings/model-settings.tsx`: show active model state and set-active action.
- Modify `src/components/settings/model-settings.test.tsx`: cover active model UI.

## Task 1: Generic Rust Catalog And Active Model Store

**Files:**
- Modify: `src-tauri/src/transcription/model.rs`
- Modify: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [x] **Step 1: Write failing catalog and active-model tests**

Add tests asserting:

```rust
#[test]
fn catalog_entry_declares_family_artifact_format_and_runtime_support() {
    let entry = parakeet_v3_catalog_entry();

    assert_eq!(entry.family, TranscriptionModelFamily::Parakeet);
    assert_eq!(entry.artifact_format, TranscriptionModelArtifactFormat::CoreMlBundle);
    assert!(entry.supported_runtimes.contains(&TranscriptionRuntimeId::CoreMl));
}

#[test]
fn store_defaults_active_model_to_first_catalog_entry() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());

    let active = store.active_model_id().expect("active model");

    assert_eq!(active, "nvidia/parakeet-tdt-0.6b-v3");
}

#[test]
fn store_marks_active_model_in_list_and_can_change_active_model() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();

    store.set_active_model(entry.id).expect("set active");
    let statuses = store.list().expect("list");

    assert!(statuses.iter().any(|status| status.model_id == entry.id && status.is_active));
}
```

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models catalog_entry_declares_family_artifact_format_and_runtime_support store_defaults_active_model_to_first_catalog_entry store_marks_active_model_in_list_and_can_change_active_model --offline
```

Expected: FAIL because the metadata and active-model APIs do not exist.

- [x] **Step 2: Implement generic catalog metadata**

Add:

```rust
pub enum TranscriptionModelFamily { Parakeet }
pub enum TranscriptionModelArtifactFormat { CoreMlBundle, Transformers }
pub enum TranscriptionRuntimeId { CoreMl, TransformersPython }
```

Add fields to `TranscriptionModelCatalogEntry`: `family`, `artifact_format`, `supported_runtimes`.

Add:

```rust
pub fn transcription_model_catalog() -> Vec<TranscriptionModelCatalogEntry> {
    vec![parakeet_v3_catalog_entry()]
}
```

- [x] **Step 3: Implement active-model state in the store**

Add `active_model_id: Arc<Mutex<String>>` to `TranscriptionModelStore`, defaulting to the first catalog entry id. Add methods:

```rust
pub fn active_model_id(&self) -> Result<String, ModelStoreError>;
pub fn active_status(&self) -> Result<TranscriptionModelStatus, ModelStoreError>;
pub fn set_active_model(&self, model_id: &str) -> Result<TranscriptionModelStatus, ModelStoreError>;
```

Add `is_active: bool` to `TranscriptionModelStatus`.

- [x] **Step 4: Verify and commit**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models --offline
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Commit:

```bash
rtk git add src-tauri/src/transcription/model.rs src-tauri/src/transcription/store.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: add generic transcription model catalog"
```

## Task 2: Model-Aware Runtime Selection And Commands

**Files:**
- Modify: `src-tauri/src/transcription/runtime.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/transcription_models.rs`

- [x] **Step 1: Write failing runtime support tests**

Add tests asserting Core ML runtime supports the Core ML catalog entry, Transformers runtime does not, and runtime selection accepts a model entry.

- [x] **Step 2: Implement model-aware runtime support**

Change `TranscriptionRuntime` to:

```rust
fn supports(&self, model: &TranscriptionModelCatalogEntry) -> bool;
fn probe(&self, model: &TranscriptionModelCatalogEntry) -> RuntimeCapability;
```

Rename concrete types:

- `NativeParakeetRuntime` -> `CoreMlParakeetRuntime`
- `PythonParakeetRuntime` -> `TransformersPythonRuntime`

Keep behavior: Core ML runtime is supported but unsupported/unfinished; Python runtime only supports `Transformers` artifact format.

- [x] **Step 3: Add active model commands**

Add Tauri commands:

```rust
get_active_transcription_model() -> TranscriptionModelStatus
set_active_transcription_model(model_id: String) -> TranscriptionModelStatus
get_transcription_runtime_status(model_id: Option<String>) -> RuntimeSelection
```

- [x] **Step 4: Verify and commit**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models --offline
rtk cargo test --manifest-path src-tauri/Cargo.toml --offline
```

Commit:

```bash
rtk git add src-tauri/src/transcription/runtime.rs src-tauri/src/main.rs src-tauri/tests/transcription_models.rs
rtk git commit -m "feat: select runtime by transcription model"
```

## Task 3: Generic Frontend Contracts And UI Copy

**Files:**
- Modify: `src/lib/transcription-models.ts`
- Modify: `src/App.tsx`
- Modify: `src/components/settings/model-settings.tsx`
- Modify: `src/components/settings/model-settings.test.tsx`
- Modify: `src/components/workspace/agent-panel.tsx`
- Modify: `src/components/workspace/agent-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`

- [x] **Step 1: Write failing UI tests**

Update tests so blocked editor copy expects `Local transcription model required.` and settings shows an `Active` marker plus a `Use model` action for inactive entries.

- [x] **Step 2: Update frontend adapter**

Add `isActive: boolean` to `TranscriptionModelStatus`, plus:

```ts
export function getActiveTranscriptionModel(): Promise<TranscriptionModelStatus>;
export function setActiveTranscriptionModel(modelId: string): Promise<TranscriptionModelStatus>;
```

- [x] **Step 3: Update app and settings UI**

Use the active model for readiness and runtime status. Rename props from `modelReady` to `transcriptionModelReady` and keep UI text generic.

- [x] **Step 4: Verify and commit**

Run:

```bash
rtk pnpm test -- src/components/workspace/agent-panel.test.tsx src/components/settings/model-settings.test.tsx
rtk pnpm lint
```

Commit:

```bash
rtk git add src/lib/transcription-models.ts src/App.tsx src/components/settings/model-settings.tsx src/components/settings/model-settings.test.tsx src/components/workspace/agent-panel.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/editor-workspace.tsx
rtk git commit -m "feat: generalize transcription model UI"
```

## Task 4: Full Verification

Run:

```bash
rtk pnpm run verify
```

Expected: PASS.

If browser-visible UI changed, run the Tauri or Vite app and verify the editor blocked copy and Settings active marker at desktop and mobile widths.
