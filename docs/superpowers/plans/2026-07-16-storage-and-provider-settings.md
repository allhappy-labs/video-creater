# Storage and Provider Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Storage’s pseudo-policy and developer commands with native folder selection, trustworthy inventory, free-space checks, and safe cleanup while making provider configuration easier to act on without weakening Keychain boundaries.

**Architecture:** Rust owns filesystem inventory, volume state, and cleanup allowlists. React owns only user preference presentation and native folder selection. Provider secrets remain write-only through existing credential commands; health aggregates only non-secret status and validation metadata.

**Tech Stack:** Rust filesystem APIs, Tauri dialog plugin, React, TypeScript, Keychain credential adapter, Vitest.

## Global Constraints

- Generic cleanup may delete only paths classified as disposable.
- Never serialize, log, or read back provider secret values.
- Storage preference changes affect the suggested New Project parent only.
- Existing projects are never moved by Settings.
- Environment credentials are read-only fallbacks.

---

## Task 1: Version the storage preference and remove the pseudo-policy

**Files:**

- Modify: `src/lib/app-settings.ts`
- Modify: `src/lib/app-settings.test.ts`
- Modify: `src/App.tsx`
- Modify: `src/components/workspace/project-home.tsx`
- Modify: `src/components/workspace/project-home.test.tsx`

- [ ] Replace the string sentinel with:

```ts
export type ProjectLocationPreference =
  | { mode: "ask" }
  | { mode: "suggestedParent"; parentPath: string };

export interface AppSettingsPreferences {
  projectLocation: ProjectLocationPreference;
  // existing non-update fields
}
```

- [ ] Write migration tests:

- missing/`"Split project folder"` => `{ mode: "ask" }`;
- absolute legacy path => suggested parent;
- relative/blank legacy path => ask;
- all unrelated preferences preserved.

- [ ] Change Project Home to use the suggested parent only as the initial
folder in the create/open dialog; never treat it as a project folder itself.

- [ ] Commit:

```bash
rtk git add src/lib/app-settings* src/App.tsx src/components/workspace/project-home*
rtk git commit -m "feat(storage): version project location preference"
```

## Task 2: Add Rust-owned storage inventory

**Files:**

- Create: `src-tauri/src/settings/storage.rs`
- Modify: `src-tauri/src/settings/health.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/settings/storage.rs`

- [ ] Define:

```rust
pub enum StorageScope {
    GlobalModels,
    DisposableAppCache,
    ProjectMedia,
    ProjectTranscripts,
    ProjectRenderArtifacts,
    ProjectWorkflowArtifacts,
}

pub struct StorageInventoryItem {
    pub id: String,
    pub scope: StorageScope,
    pub path: Option<String>,
    pub bytes: u64,
    pub free_bytes: Option<u64>,
    pub removable: bool,
    pub unavailable_reason: Option<String>,
}
```

- [ ] Write failing tests using temporary fixtures and symlinks. Inventory must
not follow a symlink outside its allowed root.

- [ ] Implement bounded directory walking, volume free-space lookup, and
`Open a project to inspect` state for project scopes without an active project.

- [ ] Add:

```rust
get_storage_health(active_project_dir: Option<String>)
refresh_storage_inventory(active_project_dir: Option<String>)
```

Refresh is a shared Settings operation.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings src-tauri/src/main.rs
rtk git commit -m "feat(settings): report storage inventory"
```

## Task 3: Implement allowlisted cleanup

**Files:**

- Modify: `src-tauri/src/settings/storage.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/settings/storage.rs`

- [ ] Define explicit cleanup targets:

```rust
pub enum StorageCleanupTarget {
    DisposableAppCache,
    ProjectRenderArtifacts { artifact_ids: Vec<String> },
}
```

- [ ] Write failing tests proving cleanup refuses:

- project manifests and timeline files;
- imported/generated media;
- transcripts;
- model roots;
- accepted render outputs not named by artifact ID;
- paths escaping through `..` or symlinks.

- [ ] Add `preview_storage_cleanup(target)` returning exact paths and bytes.
Add `run_storage_cleanup(target, confirmation_token)` and require the token
from the preview to prevent stale/broadened cleanup.

- [ ] Run cleanup through the shared operation contract with progress by item,
then refresh storage health.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings/storage.rs src-tauri/src/main.rs
rtk git commit -m "feat(storage): add scoped cleanup actions"
```

## Task 4: Build the Storage page

**Files:**

- Create: `src/components/settings/storage-settings.tsx`
- Create: `src/components/settings/storage-settings.test.tsx`
- Create: `src/lib/settings/storage.ts`
- Modify: `src/components/settings/model-settings.tsx`

- [ ] Write failing tests for:

- Ask for a folder;
- Choose folder native action;
- selected absolute parent path;
- Clear selection;
- inventory bytes/free space;
- project rows without an open project;
- preview confirmation listing exact cleanup targets;
- no free-text path input;
- no terminal commands.

- [ ] Use:

```ts
const selected = await open({
  directory: true,
  multiple: false,
  title: "Choose suggested project parent folder",
});
```

- [ ] Add `Reveal in Finder` through a focused native command that validates
the path belongs to a reported inventory item before opening it.

- [ ] Render low-space blocking copy with the volume and blocked operation.

- [ ] Commit:

```bash
rtk git add src/components/settings/storage-settings* src/lib/settings/storage.ts
rtk git commit -m "feat(settings): make storage actionable"
```

## Task 5: Add provider health aggregation without secrets

**Files:**

- Create: `src-tauri/src/settings/providers.rs`
- Modify: `src-tauri/src/provider_credentials.rs`
- Modify: `src-tauri/src/settings/health.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/settings/providers.rs`

- [ ] Define non-secret health:

```rust
pub struct ProviderHealth {
    pub provider: String,
    pub display_name: String,
    pub credential_source: ProviderCredentialSource,
    pub configured: bool,
    pub validation_state: ProviderValidationState,
    pub account_label: Option<String>,
    pub balance_label: Option<String>,
    pub dependent_model_ids: Vec<String>,
    pub last_checked_at: Option<String>,
    pub diagnostic_code: Option<String>,
}
```

- [ ] Write a serialization test that recursively rejects keys named
`secret`, `token`, `apiKey`, `credentialValue`, or raw fixture values.

- [ ] Reuse current Keychain status and account validation. Add bounded
`refresh_provider_health(provider?)`; never retrieve a Keychain value for
display.

- [ ] Group state rule:

- Configured: credential present and latest validation did not fail.
- Needs attention: a credential is present but rejected/unavailable, or an
  enabled dependent model requires a missing credential.
- Optional: no enabled dependent model currently requires it.

- [ ] Commit:

```bash
rtk git add src-tauri/src/settings/providers.rs src-tauri/src/provider_credentials.rs src-tauri/src/settings src-tauri/src/main.rs
rtk git commit -m "feat(settings): aggregate provider health"
```

## Task 6: Build the grouped Providers page

**Files:**

- Create: `src/components/settings/providers-settings.tsx`
- Create: `src/components/settings/providers-settings.test.tsx`
- Create: `src/lib/settings/providers.ts`
- Modify: `src/components/settings/model-settings.tsx`

- [ ] Write failing tests for Configured, Needs attention, and Optional
sections; credential source; dependent model list; inline rejection; retained
draft; environment fallback read-only state; Save/Replace/Remove/Retry/Refresh.

- [ ] Move the existing credential UI without changing Keychain command
semantics. After Save/Replace/Remove, refresh only provider health and preserve
the affected input’s focus.

- [ ] When validation rejects a newly entered value, keep the value in React
state, show the provider error inline, and do not serialize it elsewhere.

- [ ] Confirm generation model enable/disable changes refresh provider
dependency summaries.

- [ ] Commit:

```bash
rtk git add src/components/settings/providers-settings* src/lib/settings/providers.ts
rtk git commit -m "feat(settings): group provider readiness"
```

## Task 7: Slice verification

- [ ] Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk cargo test --manifest-path src-tauri/Cargo.toml settings -- --test-threads=1
```

- [ ] In the Tauri app prove folder selection, inventory refresh, exact cleanup
preview, cache-only deletion, provider Keychain save/replace/remove, environment
fallback, rejected credential recovery, and dependency regrouping.

