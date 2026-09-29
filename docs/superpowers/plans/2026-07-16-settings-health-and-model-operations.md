# Settings Health and Model Operations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Models into a real operational surface backed by the existing pinned Hugging Face acquisition path, and establish the shared Settings health and operation contracts used by every later category.

**Architecture:** Rust owns catalog truth, health, long-running operations, persistence, and recovery. Tauri commands start work and emit typed operation snapshots. React subscribes and polls through focused adapters. The first UI split preserves the existing Settings shell behavior while extracting Models.

**Tech Stack:** Rust, serde, chrono, Tauri 2 events, React 19, TypeScript, Vitest, Testing Library.

## Global Constraints

- Preserve `FluidInference/parakeet-tdt-0.6b-v3-coreml` at revision
  `aed02740059203c4a87495924f685de3722ae9ce`.
- Never expose a partial staging directory as installed.
- Primary UI must never instruct users to copy files manually.
- Manual import remains an Advanced recovery action using the native dialog.
- Every failure must have visible user copy plus a stable diagnostic code.
- No task may weaken manifest, hash, Core ML layout, or runtime validation.

---

## Task 1: Add the shared Settings health types

**Files:**

- Create: `src-tauri/src/settings/mod.rs`
- Create: `src-tauri/src/settings/health.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/settings/health.rs`

- [ ] Write a failing serialization and overall-state test:

```rust
#[test]
fn snapshot_uses_camel_case_and_rolls_up_the_worst_state() {
    let snapshot = SettingsHealthSnapshot::from_categories(
        "2026-07-16T12:00:00Z",
        vec![
            SettingsCategoryHealth::ready("models"),
            SettingsCategoryHealth::action_required("storage", "storage.lowSpace"),
        ],
    );

    assert_eq!(snapshot.overall, SettingsHealthState::ActionRequired);
    let json = serde_json::to_value(snapshot).expect("serialize");
    assert_eq!(json["generatedAt"], "2026-07-16T12:00:00Z");
    assert_eq!(json["categories"]["storage"]["state"], "actionRequired");
}
```

- [ ] Run the focused test and confirm it fails:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml settings::health::tests::snapshot_uses_camel_case
```

Expected: compile failure because `settings` and its types do not exist.

- [ ] Implement the common contract:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SettingsHealthState {
    Ready,
    ActionRequired,
    Checking,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsComponentHealth {
    pub id: String,
    pub label: String,
    pub state: SettingsHealthState,
    pub summary: String,
    pub action_id: Option<String>,
    pub action_label: Option<String>,
    pub last_checked_at: String,
    pub diagnostic_code: Option<String>,
    pub diagnostic_detail: Option<String>,
    pub provenance: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCategoryHealth {
    pub id: String,
    pub state: SettingsHealthState,
    pub items: Vec<SettingsComponentHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsHealthSnapshot {
    pub generated_at: String,
    pub overall: SettingsHealthState,
    pub categories: BTreeMap<String, SettingsCategoryHealth>,
}
```

Use an explicit severity function for rollup; do not rely on enum declaration
order. Unknown or not-yet-implemented categories must report `Unavailable` with
truthful copy, not `Ready`.

- [ ] Add `get_settings_health_snapshot` in `src-tauri/src/main.rs`. Its first
version builds real Models health and explicit `Unavailable` items for the
categories implemented by later plans:

```rust
#[tauri::command]
fn get_settings_health_snapshot(
    model_state: tauri::State<'_, TranscriptionModelStoreState>,
) -> Result<SettingsHealthSnapshot, String> {
    build_initial_settings_health_snapshot(&model_state.0)
        .map_err(|error| error.to_string())
}
```

- [ ] Register the command in `generate_handler!` and add a command-level test
that asserts the catalog-backed Models category is present when the model is
missing.

- [ ] Re-run the focused test, then all Rust tests.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings src-tauri/src/lib.rs src-tauri/src/main.rs
rtk git commit -m "feat(settings): add shared health contract"
```

## Task 2: Add the operation registry, journal, and recovery

**Files:**

- Create: `src-tauri/src/settings/operations.rs`
- Modify: `src-tauri/src/settings/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/settings/operations.rs`

- [ ] Write failing tests for state transitions and interrupted recovery:

```rust
#[test]
fn running_operation_cannot_transition_back_to_queued() { /* assert error */ }

#[test]
fn startup_reconciles_running_operation_as_interrupted() {
    let operation = fixture_operation(SettingsOperationState::Running);
    let recovered = reconcile_interrupted(operation, false);
    assert_eq!(recovered.state, SettingsOperationState::Failed);
    assert_eq!(recovered.error.unwrap().code, "settings.operation.interrupted");
}
```

- [ ] Implement:

```rust
pub const SETTINGS_OPERATION_EVENT: &str = "settings-operation";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SettingsOperationKind {
    ModelDownload,
    SpeechModelsDownload,
    HealthCheck,
    SkillRepair,
    StorageRefresh,
    StorageCleanup,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SettingsOperationState {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOperationError {
    pub code: String,
    pub message: String,
    pub recovery_action: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOperation {
    pub id: String,
    pub kind: SettingsOperationKind,
    pub target_id: String,
    pub phase: String,
    pub state: SettingsOperationState,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub unit: Option<String>,
    pub cancellable: bool,
    pub message: String,
    pub error: Option<SettingsOperationError>,
    pub started_at: String,
    pub updated_at: String,
}
```

Implement `SettingsOperationRegistry` with:

- `start(kind, target_id, total_units, unit)`;
- `update(id, transition)`;
- `request_cancel(id)`;
- `get(id)` and `list_active()`;
- atomic journal writes to
  `Application Support/com.olhapi.video-creater/settings/operations.json`;
- startup reconciliation supplied with a target-specific completion predicate.

- [ ] Manage one registry in Tauri setup and add:

```rust
#[tauri::command]
fn list_settings_operations(
    state: tauri::State<'_, SettingsOperationRegistryState>,
) -> Result<Vec<SettingsOperation>, String>
```

- [ ] Verify focused and full Rust tests.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings src-tauri/src/main.rs
rtk git commit -m "feat(settings): persist operational progress"
```

## Task 3: Publish model download progress through the shared operation

**Files:**

- Modify: `src-tauri/src/transcription/acquisition.rs`
- Modify: `src-tauri/src/transcription/store.rs`
- Modify: `src-tauri/src/transcription/model.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/transcription/acquisition.rs`
- Test: `src-tauri/src/transcription/store.rs`

- [ ] Extend the progress callback with bytes and write a failing fixture test:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDownloadProgressSnapshot {
    pub downloaded_files: u32,
    pub total_files: u32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}
```

The local HTTP fixture must prove monotonically increasing bytes, final file
count equality, and cancellation before publishing the staged model.

- [ ] Add status provenance required by the UI:

```rust
pub struct TranscriptionModelStatus {
    // existing fields
    pub installed_bytes: u64,
    pub source_repo_id: Option<String>,
    pub source_revision: Option<String>,
    pub source_license: Option<String>,
    pub artifact_format: String,
    pub runtime_id: String,
    pub last_error_code: Option<String>,
    pub last_error_detail: Option<String>,
}
```

- [ ] Add `download_with_observer` to `TranscriptionModelStore`; keep
`download` as a no-op-observer wrapper so existing callers remain source
compatible.

- [ ] Change `download_transcription_model` to return the queued
`SettingsOperation` immediately. Spawn blocking acquisition, update the
registry on every observer callback, and emit:

```rust
app.emit(SETTINGS_OPERATION_EVENT, operation.clone())
    .map_err(|error| error.to_string())?;
```

Map failures to stable codes:

- `model.source.unavailable`
- `model.download.cancelled`
- `model.manifest.invalid`
- `model.hash.mismatch`
- `model.coreml.invalid`
- `model.storage.insufficient`
- `model.download.failed`

- [ ] Add `cancel_settings_operation(operation_id)`; model cancellation must
delegate to `TranscriptionModelStore::cancel_download`.

- [ ] Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml transcription -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml settings -- --test-threads=1
```

- [ ] Commit:

```bash
rtk git add src-tauri/src/transcription src-tauri/src/settings src-tauri/src/main.rs
rtk git commit -m "feat(models): stream verified download progress"
```

## Task 4: Add typed frontend adapters and operation recovery

**Files:**

- Create: `src/lib/settings/health.ts`
- Create: `src/lib/settings/operations.ts`
- Create: `src/lib/settings/use-settings-operations.ts`
- Modify: `src/lib/transcription-models.ts`
- Test: `src/lib/settings/operations.test.ts`
- Test: `src/lib/settings/use-settings-operations.test.tsx`

- [ ] Write failing adapter tests that assert exact Tauri command names and
camel-case payloads.

- [ ] Define TypeScript mirrors of the Rust contracts and implement:

```ts
export const settingsOperationEvent = "settings-operation";

export function getSettingsHealthSnapshot() {
  return invoke<SettingsHealthSnapshot>("get_settings_health_snapshot");
}

export function listSettingsOperations() {
  return invoke<SettingsOperation[]>("list_settings_operations");
}

export function cancelSettingsOperation(operationId: string) {
  return invoke<SettingsOperation>("cancel_settings_operation", { operationId });
}
```

- [ ] Change:

```ts
export function downloadTranscriptionModel(modelId: string) {
  return invoke<SettingsOperation>("download_transcription_model", { modelId });
}
```

- [ ] Implement `useSettingsOperations` so it first polls
`list_settings_operations`, then subscribes with
`listen<SettingsOperation>(settingsOperationEvent, ...)`, replaces entries by
ID, and unlistens on cleanup.

- [ ] Run:

```bash
rtk pnpm test -- src/lib/settings
rtk pnpm lint
```

- [ ] Commit:

```bash
rtk git add src/lib/settings src/lib/transcription-models.ts
rtk git commit -m "feat(settings): add typed operation adapters"
```

## Task 5: Split the Settings shell and implement the operational Models page

**Files:**

- Create: `src/components/settings/settings-shell.tsx`
- Create: `src/components/settings/settings-status.tsx`
- Create: `src/components/settings/settings-diagnostics.tsx`
- Create: `src/components/settings/models-settings.tsx`
- Create: `src/components/settings/models-settings.test.tsx`
- Modify: `src/components/settings/model-settings.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

- [ ] Move navigation and responsive tab behavior unchanged into
`SettingsShell`; preserve the six categories, 220px desktop rail, horizontal
narrow rail, roving focus, `aria-controls`, and focus after actions.

- [ ] Write failing Models tests for:

1. Missing Parakeet renders `Download model`.
2. Clicking it calls `onDownload(modelId)` exactly once.
3. Active progress renders bytes/files and `Cancel`.
4. Failure renders inline error plus `Retry download`.
5. Ready renders `Verify`, `Remove`, and `Use` only when inactive.
6. Diagnostics contain repo, revision, license, runtime, path, and error code.
7. Empty catalog says `Model catalog unavailable`, not “no models installed.”
8. No primary copy contains “copy the compiled Core ML bundle.”
9. Generation model availability preferences remain functional and refresh
   provider dependency summaries.

- [ ] Add `onDownload: ModelAction` to the page contract and wire App:

```tsx
onDownload={(modelId) =>
  runModelAction(downloadTranscriptionModel, modelId)
}
```

Import `downloadTranscriptionModel` in `src/App.tsx`.

- [ ] Render operation progress from `useSettingsOperations`; use
`aria-live="polite"` for phase changes and preserve buttons during polling
recovery.

- [ ] When a model operation reaches `succeeded`, `failed`, or `cancelled`,
refresh the catalog and runtime status exactly once so the row cannot remain
stuck on its pre-operation state after the event stream becomes terminal.

- [ ] Move manual import under `<SettingsDiagnostics label="Advanced recovery">`.
Use `open({ directory: true, multiple: false })`, then call
`importTranscriptionModel(modelId, selectedPath)`.

- [ ] Keep model removal behind a confirmation dialog naming the exact model
and stating that other models/projects are unaffected.

- [ ] Run:

```bash
rtk pnpm test -- src/components/settings/models-settings.test.tsx src/App.test.tsx
rtk pnpm lint
```

- [ ] Commit:

```bash
rtk git add src/components/settings src/App.tsx src/App.test.tsx
rtk git commit -m "feat(settings): make model setup operational"
```

## Task 6: Move speech-analysis models onto the shared operation contract

**Files:**

- Modify: `src-tauri/src/speech_models.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/lib/transcription-models.ts`
- Modify: `src/components/settings/models-settings.tsx`
- Modify: `src/components/settings/models-settings.test.tsx`
- Test: `src-tauri/src/speech_models.rs`

- [ ] Add failing Rust tests for progress, retry, verification, and distinct
readiness from transcription.

- [ ] Return a queued `SettingsOperation` from
`download_production_speech_models`; emit file/byte progress and stable errors.

- [ ] Add verify and remove commands scoped to the production speech model
root. If the existing downloader cannot safely cancel between files, expose
`cancellable: false` rather than a non-functional Cancel button.

- [ ] Add frontend actions `Install`, `Retry`, `Verify`, and `Remove`, pinned
revision/license diagnostics, and independent runtime copy.

- [ ] Run focused frontend/Rust tests, then `rtk pnpm verify`.

- [ ] Commit:

```bash
rtk git add src-tauri/src/speech_models.rs src-tauri/src/main.rs src/lib/transcription-models.ts src/components/settings
rtk git commit -m "feat(models): operationalize speech analysis setup"
```

## Task 7: Slice verification

- [ ] Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

- [ ] Launch the Tauri app, delete only the test Parakeet installation if
needed, and prove:

1. catalog row remains visible;
2. `Download model` starts the pinned Hugging Face download;
3. progress updates without reopening Settings;
4. Cancel stops publication;
5. retry completes;
6. verification produces Ready;
7. transcription runtime health remains separate.

- [ ] Record any manual evidence under
`output/settings-readiness/models/` and do not commit downloaded model blobs.
