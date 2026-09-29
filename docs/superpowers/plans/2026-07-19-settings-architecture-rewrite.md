# Settings Architecture Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mixed Settings health dashboard with scoped App Settings, Project Settings, and System Health surfaces that use Keychain-only provider credentials, compact model selection, actionable missing-configuration links, and truthful native behavior.

**Architecture:** Rust owns schema-versioned app preferences, Keychain credentials, project mutations, health aggregation, and filesystem safety. React owns typed presentation and navigation through separate state owners for preferences, models, integrations, and diagnostics; editor blockers deep-link through a typed settings target. Existing model operations and bundled runtime probes remain intact but move to the correct surface.

**Tech Stack:** Tauri 2, Rust 2021, React 19, strict TypeScript, Vitest/Testing Library, shadcn primitives, Tailwind CSS, macOS Security.framework Keychain, native Tauri menu APIs.

## Global Constraints

- Work directly on `main`; the user explicitly authorized this repository workflow.
- Prefix every shell command with `rtk`.
- Use red-first tests for every behavior change and Conventional Commits for every task.
- Preserve bundled GStreamer/GES, AVFoundation helpers, model helpers, and required native runtimes; introduce no Homebrew or package-manager runtime dependency.
- Provider credentials are stored and resolved through macOS Keychain only. Product contracts and UI contain no provider environment-variable setting or fallback.
- Keep TypeScript strict with `allowJs: false`, `noUncheckedIndexedAccess`, and `exactOptionalPropertyTypes`; all new frontend source is TypeScript/TSX.
- Add no Clippy suppression. Use typed request/context structures when signatures grow.
- Do not expose linked media, proxies, optimized media, color management, background rendering, automatic transcription, audio-device controls, light theme, or updater controls without their complete underlying runtime.
- Timeline snapping remains the non-persisted pressed-state toggle in the timeline actions row and never becomes an App Setting.
- Preserve unrelated work unless the user explicitly authorizes it. The progress-file cleanup in this plan is explicitly authorized.
- Use the existing compact dark desktop visual language, shadcn primitives, Tailwind tokens, and Lucide icons. Add no custom icon family or hand-drawn SVG.
- A disabled implemented capability must show its missing prerequisite and a typed `Configure in Settings` action.
- Native visual acceptance uses the freshly built Tauri `.app`, not a browser approximation.

---

## File Structure

### Rust ownership

- Create `src-tauri/src/settings/preferences.rs`: versioned preferences, validation, atomic persistence, and legacy migration.
- Modify `src-tauri/src/settings/mod.rs`: export the preferences module.
- Modify `src-tauri/src/main.rs`: initialize preferences state and register preference, project-settings, and health commands.
- Modify `src-tauri/src/provider_credentials.rs`: Keychain-only provider credential status and resolution.
- Modify `src-tauri/src/settings/providers.rs`: neutral optional-provider aggregation without environment sources.
- Modify `src-tauri/src/settings/health.rs`: required-only System Health rollup and neutral context semantics.
- Modify `src-tauri/src/project/action.rs`: one validated project-settings mutation.
- Modify `src-tauri/src/project/split.rs`: persist and validate the project-settings mutation across split sidecars.
- Modify `src-tauri/src/native_menu.rs`: Project Settings and System Health commands; remove dead updater/save commands.
- Modify the exact provider workflow, sidecar, fixture, and test files enumerated in Task 3 to remove credential environment-variable fields from product request contracts.

### Frontend ownership

- Rewrite `src/lib/app-settings.ts`: typed IPC adapter and one-time v1 migration reader.
- Create `src/lib/settings/target.ts`: typed deep-link targets.
- Create `src/components/settings/configuration-notice.tsx`: reusable missing-configuration notice.
- Rewrite `src/components/settings/settings-shell.tsx`: App Settings categories without health badges.
- Split `src/components/settings/settings.tsx`: small App Settings owner with presentational pages.
- Create `src/components/settings/projects-settings.tsx`: new-project defaults.
- Refactor `src/components/settings/models-settings.tsx`: local speech/model management only.
- Create `src/components/settings/generation-model-multiselect.tsx`: compact searchable checkbox combobox.
- Refactor `src/components/settings/providers-settings.tsx`: Keychain integration rows only.
- Create `src/components/settings/advanced-settings.tsx`: execution, MCP, and recovery.
- Create `src/components/settings/system-health.tsx`: diagnostic-only surface.
- Create `src/components/settings/project-settings.tsx`: project format/output, storage, and guidance.
- Modify `src/App.tsx`: separate surfaces, async preferences, deep links, and focus restoration.
- Modify `src/components/workspace/editor-workspace.tsx` and affected generation/transcription controls: contextual configuration notices.
- Modify `src/components/workspace/project-home.tsx`: remove fake traffic lights.
- Modify `src/lib/native-menu.ts`: updated native commands.
- Modify `vite.config.ts` and `src/vite-build-policy.test.ts`: remove the circular first-party chunk boundary.

### Verification ownership

- Update focused existing tests beside every modified file.
- Add `src/components/settings/configuration-notice.test.tsx`.
- Add `src/components/settings/generation-model-multiselect.test.tsx`.
- Add `src/components/settings/project-settings.test.tsx`.
- Add `src/components/settings/system-health.test.tsx`.
- Update native Settings fixtures, acceptance runner, and screenshot scenarios only after product behavior passes focused tests.

---

### Task 1: Rust-Owned App Preferences

**Files:**
- Create: `src-tauri/src/settings/preferences.rs`
- Modify: `src-tauri/src/settings/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/settings/preferences.rs`
- Test: `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: Tauri `app_data_dir`, existing `ProjectLocationPreference` semantics, project `CaptionRenderMode`.
- Produces: `AppPreferencesState`, `AppPreferencesV2`, `LegacyAppPreferencesV1`, `AppPreferencesPatch`, `get_app_preferences`, and `update_app_preferences`.

- [ ] **Step 1: Write failing persistence and migration tests**

```rust
#[test]
fn missing_store_uses_safe_v2_defaults() {
    let root = tempfile::tempdir().unwrap();
    let store = AppPreferencesStore::new(root.path().join("preferences.json"));
    let preferences = store.load_or_migrate(None).unwrap();
    assert_eq!(preferences.schema_version, 2);
    assert!(preferences.require_provider_upload_confirmation);
    assert!(!preferences.render_completion_notifications);
    assert!(preferences.enabled_generation_model_ids.is_empty());
    assert_eq!(preferences.new_project_defaults.width, 1920);
    assert_eq!(preferences.new_project_defaults.height, 1080);
    assert_eq!(preferences.new_project_defaults.fps, 30.0);
}

#[test]
fn v1_migration_drops_environment_and_negative_model_fields() {
    let root = tempfile::tempdir().unwrap();
    let store = AppPreferencesStore::new(root.path().join("preferences.json"));
    let migrated = store.load_or_migrate(Some(LegacyAppPreferencesV1 {
        provider_credential_env_var: Some("FAL_KEY".into()),
        disabled_generation_model_ids: vec![],
        require_provider_upload_confirmation: Some(false),
        render_completion_notifications: Some(true),
        project_location: Some(ProjectLocationPreference::Ask),
        generation_execution_backend: Some(GenerationExecutionBackend::InProcess),
    })).unwrap();
    assert!(!migrated.require_provider_upload_confirmation);
    assert!(migrated.render_completion_notifications);
    assert!(migrated.enabled_generation_model_ids.is_empty());
    let serialized = serde_json::to_string(&migrated).unwrap();
    assert!(!serialized.contains("providerCredentialEnvVar"));
    assert!(!serialized.contains("disabledGenerationModelIds"));
}

#[test]
fn writes_are_atomic_and_invalid_dimensions_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let store = AppPreferencesStore::new(root.path().join("preferences.json"));
    let error = store.update(AppPreferencesPatch {
        new_project_defaults: Some(NewProjectDefaults { width: 0, height: 1080, fps: 30.0, loudness_lufs: -14.0, captions: CaptionRenderMode::BurnIn }),
        ..Default::default()
    }).unwrap_err();
    assert_eq!(error.code(), "settings.preferences.invalidDimensions");
    assert!(!root.path().join("preferences.json").exists());
}
```

- [ ] **Step 2: Run the focused tests and confirm red state**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml settings::preferences::tests -- --nocapture`

Expected: compilation fails because the preferences module and types do not exist.

- [ ] **Step 3: Implement the store and typed commands**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferencesV2 {
    pub schema_version: u32,
    pub project_location: ProjectLocationPreference,
    pub require_provider_upload_confirmation: bool,
    pub render_completion_notifications: bool,
    pub new_project_defaults: NewProjectDefaults,
    pub enabled_generation_model_ids: Vec<String>,
    pub generation_execution_backend: GenerationExecutionBackend,
}

pub struct AppPreferencesStore { path: PathBuf }
pub struct AppPreferencesState(pub Mutex<AppPreferencesStore>);

impl AppPreferencesStore {
    pub fn load_or_migrate(&self, legacy: Option<LegacyAppPreferencesV1>) -> Result<AppPreferencesV2, AppPreferencesError>;
    pub fn update(&self, patch: AppPreferencesPatch) -> Result<AppPreferencesV2, AppPreferencesError>;
    fn write_atomic(&self, preferences: &AppPreferencesV2) -> Result<(), AppPreferencesError>;
}

#[tauri::command]
fn get_app_preferences(
    state: State<'_, AppPreferencesState>,
    legacy: Option<LegacyAppPreferencesV1>,
) -> Result<AppPreferencesV2, AppPreferencesCommandError>;

#[tauri::command]
fn update_app_preferences(
    state: State<'_, AppPreferencesState>,
    patch: AppPreferencesPatch,
) -> Result<AppPreferencesV2, AppPreferencesCommandError>;
```

Initialize the state at `app_data_dir.join("settings/preferences.json")`; validate finite FPS/LUFS, even dimensions in the supported range, canonical model IDs, and normalized absolute project paths. Use `NamedTempFile`, `sync_all`, and persist/rename as in project storage.

- [ ] **Step 4: Verify Rust behavior and command registration**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml settings::preferences::tests -- --nocapture`

Expected: all preference tests pass.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml app_preferences_command -- --nocapture`

Expected: Tauri command tests serialize schema version 2 and structured validation errors.

- [ ] **Step 5: Commit Task 1**

```bash
rtk git add src-tauri/src/settings/preferences.rs src-tauri/src/settings/mod.rs src-tauri/src/main.rs
rtk git commit -m "feat(settings): persist native app preferences"
```

### Task 2: Strict Frontend Preferences and New-Project Defaults

**Files:**
- Rewrite: `src/lib/app-settings.ts`
- Modify: `src/lib/app-settings.test.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/components/workspace/project-home.tsx`
- Test: `src/components/workspace/project-home.test.tsx`

**Interfaces:**
- Consumes: Task 1 `get_app_preferences` and `update_app_preferences` commands.
- Produces: `AppPreferences`, `AppPreferencesPatch`, `loadAppPreferences`, `updateAppPreferences`, and `createEmptySplitProject(projectDir, defaults)`.

- [ ] **Step 1: Write failing adapter and creation tests**

```ts
it("sends legacy v1 once and accepts the Rust v2 authority", async () => {
  localStorage.setItem("video-creater.appSettings.v1", JSON.stringify({
    providerCredentialEnvVar: "FAL_KEY",
    disabledGenerationModelIds: [],
    renderCompletionNotifications: true,
  }));
  mockInvoke.mockResolvedValueOnce(appPreferences({ renderCompletionNotifications: true }));
  await expect(loadAppPreferences()).resolves.toMatchObject({
    schemaVersion: 2,
    renderCompletionNotifications: true,
    enabledGenerationModelIds: [],
  });
  expect(mockInvoke).toHaveBeenCalledWith("get_app_preferences", {
    legacy: expect.not.objectContaining({ providerCredentialEnvVar: expect.anything() }),
  });
  expect(localStorage.getItem("video-creater.appSettings.v1")).toBeNull();
});

it("copies native project defaults into only the new project", () => {
  const project = createEmptySplitProject("/tmp/new", {
    width: 3840, height: 2160, fps: 24, loudnessLufs: -16, captions: "mux",
  });
  expect(project.renderSettings).toEqual({
    width: 3840, height: 2160, fps: 24, loudnessLufs: -16, captions: "mux",
  });
});
```

- [ ] **Step 2: Run focused tests and confirm they fail**

Run: `rtk pnpm exec vitest run src/lib/app-settings.test.ts src/App.test.tsx src/components/workspace/project-home.test.tsx`

Expected: failures show the synchronous localStorage API and hard-coded project defaults.

- [ ] **Step 3: Implement the strict adapter and async app bootstrap**

```ts
export interface AppPreferences {
  schemaVersion: 2;
  projectLocation: ProjectLocationPreference;
  requireProviderUploadConfirmation: boolean;
  renderCompletionNotifications: boolean;
  newProjectDefaults: NewProjectDefaults;
  enabledGenerationModelIds: string[];
  generationExecutionBackend: "inProcess" | "temporal";
}

export async function loadAppPreferences(): Promise<AppPreferences> {
  const legacy = readLegacyPreferencesWithoutSecrets();
  const accepted = await invoke<AppPreferences>("get_app_preferences", { legacy });
  window.localStorage.removeItem(appSettingsStorageKey);
  return accepted;
}

export function updateAppPreferences(patch: AppPreferencesPatch) {
  return invoke<AppPreferences>("update_app_preferences", { patch });
}
```

Bootstrap preferences once in `App`, render a bounded loading state before Home/Settings needs them, pass `newProjectDefaults` into project creation, and update the suggested-parent flow from the accepted Rust value.

- [ ] **Step 4: Verify focused frontend behavior and strict types**

Run: `rtk pnpm exec vitest run src/lib/app-settings.test.ts src/App.test.tsx src/components/workspace/project-home.test.tsx`

Expected: focused tests pass with no localStorage preference write.

Run: `rtk pnpm lint`

Expected: both strict TypeScript configurations pass.

- [ ] **Step 5: Commit Task 2**

```bash
rtk git add src/lib/app-settings.ts src/lib/app-settings.test.ts src/App.tsx src/App.test.tsx src/components/workspace/project-home.tsx src/components/workspace/project-home.test.tsx
rtk git commit -m "refactor(settings): load native preferences"
```

### Task 3: Keychain-Only Provider Runtime

**Files:**
- Modify: `src-tauri/src/provider_credentials.rs`
- Modify: `src-tauri/src/settings/providers.rs`
- Modify: `src-tauri/src/settings/health.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/workflows/mod.rs`
- Modify: `src-tauri/src/codex/tools.rs`
- Modify: `src-tauri/src/project/split.rs`
- Modify: `src/lib/project.ts`
- Modify: `src/lib/provider-account.ts`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src-tauri/src/bin/video-creater-provider-app-e2e.rs`
- Modify: `src-tauri/src/bin/video-creater-provider-cancel-e2e.rs`
- Modify: `src-tauri/src/bin/video-creater-provider-e2e.rs`
- Modify: `src-tauri/src/bin/video-creater-settings-e2e.rs`
- Modify: `src/App.test.tsx`
- Modify: `src/components/workspace/agent-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Modify: `src/components/workspace/project-timeline-inspector.test.tsx`
- Modify: `src/components/workspace/source-clip-inspector.test.tsx`
- Modify: `src/lib/app-settings.ts`
- Modify: `src/lib/app-settings.test.ts`
- Modify: `src/lib/modern-editor-visual-qa-fixtures.ts`
- Modify: `src/lib/project.test.ts`
- Modify: `src/lib/sample-project.ts`
- Test: `src-tauri/src/provider_credentials.rs`
- Test: `src-tauri/src/settings/providers.rs`
- Test: `src-tauri/src/settings/health.rs`
- Test: `src-tauri/src/workflows/mod.rs`
- Test: `src-tauri/src/project/split.rs`

**Interfaces:**
- Consumes: stable provider IDs and existing Security.framework Keychain store.
- Produces: `ProviderCredentialSource::{Keychain, Missing, Unavailable}`, secret-free `ProviderCredentialStatus`, and `resolve_provider_credential(provider)` with no environment fallback.

- [ ] **Step 1: Replace fallback expectations with failing Keychain-only tests**

```rust
#[test]
fn missing_keychain_item_stays_missing_even_when_process_environment_has_a_key() {
    let environment = FakeEnvironment(HashMap::from([("OPENAI_API_KEY".into(), "ignored".into())]));
    let status = status_with(&FakeStore::default(), &environment, "openai").unwrap();
    assert_eq!(status.source, ProviderCredentialSource::Missing);
    assert!(!status.configured);
    assert!(serde_json::to_value(status).unwrap().get("envVar").is_none());
}

#[test]
fn resolver_never_returns_an_environment_credential() {
    let environment = FakeEnvironment(HashMap::from([("FAL_KEY".into(), "ignored".into())]));
    let error = resolve_with(&FakeStore::default(), &environment, "fal.ai").unwrap_err();
    assert_eq!(error.code(), "providers.credentialMissing");
}
```

Add workflow serialization tests asserting `providerCredentialEnvVar` and `credentialEnvVar` are absent from new requests and project sidecars.

- [ ] **Step 2: Run provider and workflow tests to confirm red state**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml provider_credentials::tests -- --nocapture`

Expected: old environment fallback tests or new assertions fail.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml settings::providers::tests -- --nocapture`

Expected: old environment fallback tests or new assertions fail.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml provider_request_credential_contract -- --nocapture`

Expected: serialized workflow requests still contain the legacy environment field.

- [ ] **Step 3: Remove environment fields and route all product resolution through Keychain**

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderCredentialSource { Keychain, Missing, Unavailable }

pub struct ResolvedProviderCredential {
    provider: &'static str,
    secret: String,
}

fn status_for_definition(
    store: &impl CredentialStore,
    definition: &ProviderDefinition,
) -> ProviderCredentialStatus {
    match store.get(definition.provider) {
        Ok(Some(value)) if valid_stored_credential(&value) => ProviderCredentialStatus::keychain(definition),
        Ok(None) | Ok(Some(_)) => ProviderCredentialStatus::missing(definition),
        Err(_) => ProviderCredentialStatus::unavailable(definition),
    }
}
```

Keep the acceptance-only Keychain service namespace override internal to the Keychain adapter. Remove provider env-var names from status payloads, account payloads, generation briefs, Temporal start requests, Codex tool inputs, split sidecars, and frontend types. Update all real generation call sites to resolve by provider ID.

- [ ] **Step 4: Verify the contract and scan for product leaks**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml provider_credentials::tests -- --nocapture`

Expected: Keychain-only credential tests pass.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml settings::providers::tests -- --nocapture`

Expected: Keychain-only tests pass.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml provider_request_credential_contract -- --nocapture`

Expected: new requests and sidecars contain provider IDs but no credential environment fields.

Run: `rtk rg -n "providerCredentialEnvVar|provider_credential_env_var|ProviderCredentialSource::Environment" src src-tauri/src --glob '!src-tauri/src/bin/*e2e*'`

Expected: no product-runtime matches. Acceptance-only Keychain service names may remain.

- [ ] **Step 5: Commit Task 3**

```bash
rtk git add src-tauri/src/provider_credentials.rs src-tauri/src/settings/providers.rs src-tauri/src/settings/health.rs src-tauri/src/main.rs src-tauri/src/workflows/mod.rs src-tauri/src/codex/tools.rs src-tauri/src/project/split.rs src-tauri/src/bin/video-creater-provider-app-e2e.rs src-tauri/src/bin/video-creater-provider-cancel-e2e.rs src-tauri/src/bin/video-creater-provider-e2e.rs src-tauri/src/bin/video-creater-settings-e2e.rs src/lib/project.ts src/lib/project.test.ts src/lib/provider-account.ts src/lib/app-settings.ts src/lib/app-settings.test.ts src/lib/sample-project.ts src/lib/modern-editor-visual-qa-fixtures.ts src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/agent-panel.test.tsx src/components/workspace/project-timeline-inspector.test.tsx src/components/workspace/source-clip-inspector.test.tsx
rtk git commit -m "refactor(providers): require keychain credentials"
```

### Task 4: Scoped Settings Navigation and Deep-Link Contract

**Files:**
- Create: `src/lib/settings/target.ts`
- Modify: `src/components/settings/settings-shell.tsx`
- Modify: `src/components/settings/settings.tsx`
- Modify: `src/components/settings/settings-accessibility.test.tsx`
- Modify: `src/components/settings/settings.integration.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

**Interfaces:**
- Consumes: Task 2 `AppPreferences`.
- Produces: `AppSettingsCategory`, `AppSettingsTarget`, `openAppSettings(target, origin)`, and an App Settings shell without health state.

- [ ] **Step 1: Write failing shell, unmount, and target-focus tests**

```tsx
it("renders preference categories without health badges", () => {
  render(<SettingsShell initialCategory="general" onBack={vi.fn()}>{() => null}</SettingsShell>);
  expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
    "General", "Projects", "AI & Models", "Integrations", "Storage", "Advanced",
  ]);
  expect(screen.queryByText(/Ready|Failed|Unavailable|Action required/)).not.toBeInTheDocument();
});

it("unmounts settings and restores the exact origin", async () => {
  render(<App />);
  const origin = await screen.findByRole("button", { name: "Configure provider" });
  origin.focus();
  fireEvent.click(origin);
  expect(await screen.findByRole("heading", { name: "Integrations" })).toHaveFocus();
  fireEvent.click(screen.getByRole("button", { name: "Back" }));
  expect(screen.queryByTestId("settings-shell")).not.toBeInTheDocument();
  expect(origin).toHaveFocus();
});
```

- [ ] **Step 2: Run focused tests and confirm current shell fails**

Run: `rtk pnpm exec vitest run src/components/settings/settings-accessibility.test.tsx src/components/settings/settings.integration.test.tsx src/App.test.tsx`

Expected: old categories/badges render and Settings remains hidden-mounted.

- [ ] **Step 3: Implement typed targets and scoped App Settings shell**

```ts
export type AppSettingsTarget =
  | { category: "general"; item?: "privacy" | "notifications" }
  | { category: "projects"; item?: "newProjectDefaults" }
  | { category: "aiModels"; item?: "transcription" | "generationModels" }
  | { category: "integrations"; provider?: ProviderId }
  | { category: "storage"; item?: "projectLocation" | "cache" }
  | { category: "advanced"; item?: "execution" | "mcp" | "recovery" };

export interface SettingsOrigin {
  element: HTMLElement | null;
  returnView: "home" | "editor";
}
```

Render Settings only when its view is active. Remove `categoryStates`, `checkingAll`, and `onCheckAll` from `SettingsShell`. On navigation request, select the category, requestAnimationFrame-scroll the target ref, focus its heading/control, and announce the destination.

- [ ] **Step 4: Verify navigation, unmounting, and keyboard behavior**

Run: `rtk pnpm exec vitest run src/components/settings/settings-accessibility.test.tsx src/components/settings/settings.integration.test.tsx src/App.test.tsx`

Expected: six new tabs pass roving-tab keyboard tests, no badge copy exists, deep links focus correctly, and Settings unmounts on Back.

- [ ] **Step 5: Commit Task 4**

```bash
rtk git add src/lib/settings/target.ts src/components/settings/settings-shell.tsx src/components/settings/settings.tsx src/components/settings/settings-accessibility.test.tsx src/components/settings/settings.integration.test.tsx src/App.tsx src/App.test.tsx
rtk git commit -m "refactor(settings): separate preference navigation"
```

### Task 5: General, Projects, Storage, and Advanced Pages

**Files:**
- Modify: `src/components/settings/general-settings.tsx`
- Modify: `src/components/settings/general-settings.test.tsx`
- Create: `src/components/settings/projects-settings.tsx`
- Create: `src/components/settings/projects-settings.test.tsx`
- Modify: `src/components/settings/storage-settings.tsx`
- Modify: `src/components/settings/storage-settings.test.tsx`
- Create: `src/components/settings/advanced-settings.tsx`
- Create: `src/components/settings/advanced-settings.test.tsx`
- Modify: `src/components/settings/agent-mcp-settings.tsx`
- Modify: `src/components/settings/agent-mcp-settings.test.tsx`
- Modify: `src/components/settings/settings.tsx`

**Interfaces:**
- Consumes: Task 2 accepted preferences and Task 4 target refs.
- Produces: preference-only General/Projects pages, global-only Storage, and an Advanced page for execution, developer integration, and recovery.

- [ ] **Step 1: Write failing neutral-state and scope tests**

```tsx
it("treats unrequested notifications as neutral", async () => {
  renderGeneral({ permissionStatus: "notDetermined", deliveryAvailable: true });
  expect(await screen.findByText("Not enabled")).toBeInTheDocument();
  expect(screen.queryByText("Action required")).not.toBeInTheDocument();
});

it("renders only global storage without a project", async () => {
  renderStorage({ items: [globalModels, appCache, projectMedia] });
  expect(await screen.findByText("Application cache")).toBeInTheDocument();
  expect(screen.queryByText("Project media")).not.toBeInTheDocument();
});

it("updates validated new-project defaults", async () => {
  const onChange = vi.fn();
  render(<ProjectsSettings preferences={preferences()} onChange={onChange} />);
  fireEvent.change(screen.getByLabelText("Project frame rate"), { target: { value: "24" } });
  expect(onChange).toHaveBeenCalledWith({ newProjectDefaults: expect.objectContaining({ fps: 24 }) });
});

it("keeps diagnostics out of Advanced", () => {
  renderAdvanced({ project: null, executionBackend: "inProcess" });
  expect(screen.getByText("Desktop execution")).toBeInTheDocument();
  expect(screen.getByText("Open a project to copy MCP configuration.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Open System Health" })).toBeInTheDocument();
  expect(screen.queryByText("GStreamer status")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the page tests and confirm red state**

Run: `rtk pnpm exec vitest run src/components/settings/general-settings.test.tsx src/components/settings/projects-settings.test.tsx src/components/settings/storage-settings.test.tsx src/components/settings/advanced-settings.test.tsx src/components/settings/agent-mcp-settings.test.tsx`

Expected: missing Projects/Advanced components, notification warning copy, project rows in global Storage, or old Agent & MCP health content fail.

- [ ] **Step 3: Implement the preference-only pages**

Use two-column setting rows, native/select controls, and accepted Rust patches. Remove Updates and Render System from General. Projects uses presets plus validated custom dimensions, FPS, LUFS, and caption mode. Storage filters Rust inventory to global scopes and links installed model removal to `{ category: "aiModels", item: "transcription" }`. Advanced owns Desktop/Temporal execution selection, project-pinned MCP configuration, `Open System Health`, log reveal, confirmed preference reset, and advanced local-model folder import. With no project, MCP configuration is neutral. Keep self-tests, protocol diagnostics, raw repair commands, and environment-variable instructions out of Advanced.

```tsx
<SettingRow label="Frame rate" description="Copied into newly created projects.">
  <select aria-label="Project frame rate" value={defaults.fps} onChange={changeFps}>
    <option value={23.976}>23.976 fps</option>
    <option value={24}>24 fps</option>
    <option value={25}>25 fps</option>
    <option value={29.97}>29.97 fps</option>
    <option value={30}>30 fps</option>
    <option value={50}>50 fps</option>
    <option value={59.94}>59.94 fps</option>
    <option value={60}>60 fps</option>
  </select>
</SettingRow>
```

- [ ] **Step 4: Verify page behavior and strict types**

Run: `rtk pnpm exec vitest run src/components/settings/general-settings.test.tsx src/components/settings/projects-settings.test.tsx src/components/settings/storage-settings.test.tsx src/components/settings/advanced-settings.test.tsx src/components/settings/agent-mcp-settings.test.tsx`

Expected: all focused page tests pass.

Run: `rtk pnpm lint`

Expected: strict TypeScript passes.

- [ ] **Step 5: Commit Task 5**

```bash
rtk git add src/components/settings/general-settings.tsx src/components/settings/general-settings.test.tsx src/components/settings/projects-settings.tsx src/components/settings/projects-settings.test.tsx src/components/settings/storage-settings.tsx src/components/settings/storage-settings.test.tsx src/components/settings/advanced-settings.tsx src/components/settings/advanced-settings.test.tsx src/components/settings/agent-mcp-settings.tsx src/components/settings/agent-mcp-settings.test.tsx src/components/settings/settings.tsx
rtk git commit -m "feat(settings): add scoped preference pages"
```

### Task 6: Compact Generation Model Multiselect

**Files:**
- Create: `src/components/settings/generation-model-multiselect.tsx`
- Create: `src/components/settings/generation-model-multiselect.test.tsx`
- Modify: `src/components/settings/models-settings.tsx`
- Modify: `src/components/settings/models-settings.test.tsx`
- Modify: `src/components/settings/settings.tsx`
- Modify: `src/lib/app-settings.ts`

**Interfaces:**
- Consumes: `GenerationSettingsModel[]`, Keychain presence metadata, `enabledGenerationModelIds`.
- Produces: `GenerationModelMultiselect` with `onChange(nextIds: string[])` and a compact summary trigger.

- [ ] **Step 1: Write failing combobox behavior tests**

```tsx
it("searches grouped models and toggles checkboxes from the keyboard", async () => {
  const onChange = vi.fn();
  render(<GenerationModelMultiselect models={models} enabledIds={[]} providerStatuses={statuses} onChange={onChange} onConfigureProvider={vi.fn()} />);
  const trigger = screen.getByRole("combobox", { name: "Enabled generation models" });
  expect(trigger).toHaveTextContent("No remote models enabled");
  fireEvent.click(trigger);
  fireEvent.change(screen.getByRole("searchbox", { name: "Search generation models" }), { target: { value: "gpt-image" } });
  const option = screen.getByRole("menuitemcheckbox", { name: /GPT-image-2/ });
  option.focus();
  fireEvent.keyDown(option, { key: " " });
  expect(onChange).toHaveBeenCalledWith(["openai:gpt-image-2"]);
});

it("keeps the popover bounded and links missing providers", () => {
  renderMultiselect({ configured: false });
  fireEvent.click(screen.getByRole("combobox", { name: "Enabled generation models" }));
  expect(screen.getByTestId("generation-model-options")).toHaveClass("max-h-80", "overflow-y-auto");
  expect(screen.getByRole("button", { name: "Configure OpenAI" })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run focused tests and confirm the component is missing**

Run: `rtk pnpm exec vitest run src/components/settings/generation-model-multiselect.test.tsx src/components/settings/models-settings.test.tsx`

Expected: module-not-found or missing combobox failures.

- [ ] **Step 3: Implement the accessible bounded multiselect**

Use the existing Popover/Command primitives when available; otherwise use the existing Button plus a focus-trapped positioned panel. Group by image/video/audio, search label/provider, expose `role="menuitemcheckbox"`, group Select all/Clear, Escape closure, and focus return. Model IDs use `provider:id` and sort before persistence.

```tsx
<Button role="combobox" aria-expanded={open} aria-controls={listId} onClick={() => setOpen(true)}>
  {generationModelSummary(models, enabledIds)}
  <ChevronsUpDown aria-hidden="true" />
</Button>
```

Remove the four-column checkbox grid from Providers. Keep local model download/status in Models and render this multiselect as the compact Generation Models section.

- [ ] **Step 4: Verify keyboard, summary, grouping, and layout**

Run: `rtk pnpm exec vitest run src/components/settings/generation-model-multiselect.test.tsx src/components/settings/models-settings.test.tsx`

Expected: all multiselect and model-manager tests pass, including Escape/focus return and provider configuration links.

- [ ] **Step 5: Commit Task 6**

```bash
rtk git add src/components/settings/generation-model-multiselect.tsx src/components/settings/generation-model-multiselect.test.tsx src/components/settings/models-settings.tsx src/components/settings/models-settings.test.tsx src/components/settings/settings.tsx src/lib/app-settings.ts
rtk git commit -m "feat(settings): compact generation model selection"
```

### Task 7: Keychain Integration Rows

**Files:**
- Modify: `src/components/settings/providers-settings.tsx`
- Modify: `src/components/settings/providers-settings.test.tsx`
- Modify: `src/lib/settings/providers.ts`
- Modify: `src/lib/settings/providers.test.ts`
- Modify: `src/components/settings/settings.tsx`

**Interfaces:**
- Consumes: Task 3 secret-free Keychain provider status and Task 6 enabled model IDs.
- Produces: configured-first integration rows with Connect/Update/Validate/Disconnect.

- [ ] **Step 1: Write failing neutral integration tests**

```tsx
it("treats an untouched provider as available rather than failed", async () => {
  renderProviders([provider({ configured: false, validationState: "missing" })]);
  expect(await screen.findByRole("heading", { name: "Available integrations" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Connect OpenAI" })).toBeInTheDocument();
  expect(screen.queryByText(/Needs attention|environment|OPENAI_API_KEY/i)).not.toBeInTheDocument();
});

it("warns before disconnecting a provider used by enabled models", async () => {
  renderProviders([provider({ configured: true, dependentModelIds: ["openai:gpt-image-2"] })]);
  fireEvent.click(screen.getByRole("button", { name: "Disconnect OpenAI" }));
  expect(screen.getByRole("dialog", { name: "Disconnect OpenAI" })).toHaveTextContent("GPT-image-2 will become unavailable");
});
```

- [ ] **Step 2: Run provider UI tests and confirm old grouping fails**

Run: `rtk pnpm exec vitest run src/components/settings/providers-settings.test.tsx src/lib/settings/providers.test.ts`

Expected: old Needs attention/environment fallback UI violates assertions.

- [ ] **Step 3: Refactor Providers into Integrations**

Render Configured and Available groups. Credential input is write-only, clears after save, and never receives a stored value. Preserve operation recency/race protections. Remove generation execution and model checkbox sections. Validation failures stay within the provider row; missing untouched credentials remain neutral.

```tsx
<form aria-label={`${provider.displayName} credential`} onSubmit={save}>
  <input type="password" autoComplete="off" value={draft} onChange={changeDraft} aria-label={`New ${provider.displayName} key`} />
  <Button type="submit">{provider.configured ? "Update key" : "Connect"}</Button>
</form>
```

- [ ] **Step 4: Verify secret redaction, grouping, and provider operations**

Run: `rtk pnpm exec vitest run src/components/settings/providers-settings.test.tsx src/lib/settings/providers.test.ts`

Expected: configured/available grouping, confirmation, validation retry, and stale-operation tests pass; no environment copy exists.

- [ ] **Step 5: Commit Task 7**

```bash
rtk git add src/components/settings/providers-settings.tsx src/components/settings/providers-settings.test.tsx src/lib/settings/providers.ts src/lib/settings/providers.test.ts src/components/settings/settings.tsx
rtk git commit -m "refactor(settings): present keychain integrations"
```

### Task 8: Diagnostic-Only System Health

**Files:**
- Create: `src/components/settings/system-health.tsx`
- Create: `src/components/settings/system-health.test.tsx`
- Modify: `src/lib/settings/health.ts`
- Create: `src/lib/settings/health.test.ts`
- Modify: `src-tauri/src/settings/health.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: existing render, model, agent, skill, storage probes and operation registry.
- Produces: required-only `SystemHealthSnapshot`, local category refresh, and `openSystemHealth(origin)`.

- [ ] **Step 1: Write failing aggregation and local-error tests**

```rust
#[test]
fn optional_and_context_items_do_not_lower_overall_readiness() {
    let snapshot = SystemHealthSnapshot::from_sections(vec![
        section("rendering", Ready, true),
        section("agent", Failed, false),
        section("project", NotApplicable, false),
    ]);
    assert_eq!(snapshot.overall, SettingsHealthState::Ready);
}
```

```tsx
it("keeps healthy sections visible when one refresh fails", async () => {
  renderSystemHealth(healthySnapshot);
  mockRefresh.mockRejectedValueOnce(new Error("agent probe failed"));
  fireEvent.click(screen.getByRole("button", { name: "Retry Agent Runtime" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("agent probe failed");
  expect(screen.getByText("GStreamer / GES")).toBeInTheDocument();
  expect(screen.getByText("Ready")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run health tests and confirm old severity/global error behavior fails**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml settings::health::tests -- --nocapture`

Expected: old max-severity aggregation marks optional/context failures globally.

Run: `rtk pnpm exec vitest run src/components/settings/system-health.test.tsx src/lib/settings/health.test.ts`

Expected: missing System Health surface or global replacement failure.

- [ ] **Step 3: Implement required-only health ownership**

```rust
pub struct SystemHealthSection {
    pub id: String,
    pub required: bool,
    pub state: SettingsHealthState,
    pub items: Vec<SettingsComponentHealth>,
}

pub enum SettingsHealthState {
    Ready,
    NeedsAction,
    Failed,
    Checking,
    NotConfigured,
    NotEnabled,
    NotApplicable,
    Unavailable,
}
```

Build Rendering, Local AI, Agent Runtime, Project Integrity, and Environment sections. Overall considers only `required == true`. Category refresh returns a section and leaves other cached sections unchanged. Move render, agent, skill, and protocol diagnostics out of App Settings.

- [ ] **Step 4: Verify Rust/frontend health and operation isolation**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml settings::health::tests -- --nocapture`

Expected: required-only and neutral-context aggregation passes.

Run: `rtk pnpm exec vitest run src/components/settings/system-health.test.tsx src/lib/settings/health.test.ts src/lib/settings/use-settings-operations.test.tsx`

Expected: local retries preserve unrelated state and operation recency tests pass.

- [ ] **Step 5: Commit Task 8**

```bash
rtk git add src/components/settings/system-health.tsx src/components/settings/system-health.test.tsx src/lib/settings/health.ts src/lib/settings/health.test.ts src/lib/settings/use-settings-operations.test.tsx src-tauri/src/settings/health.rs src-tauri/src/main.rs src/App.tsx
rtk git commit -m "feat(settings): add required system health"
```

### Task 9: Project Settings and Structured Persistence

**Files:**
- Create: `src/components/settings/project-settings.tsx`
- Create: `src/components/settings/project-settings.test.tsx`
- Modify: `src-tauri/src/project/action.rs`
- Modify: `src-tauri/src/project/split.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/lib/project.ts`
- Modify: `src/lib/project.test.ts`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: active split project, storage inventory, bundled skill verification.
- Produces: `ProjectAction::UpdateProjectSettings { name, render_settings }`, `updateProjectSettingsInSplitProjectFolder`, and Project Settings UI.

- [ ] **Step 1: Write failing action, validation, and UI tests**

```rust
#[test]
fn update_project_settings_changes_name_and_render_settings_atomically() {
    let mut project = fixtures::sample_project();
    apply_project_action(&mut project, ProjectAction::UpdateProjectSettings {
        name: "Interview cut".into(),
        render_settings: RenderSettings { width: 3840, height: 2160, fps: 24.0, loudness_lufs: -16.0, captions: CaptionRenderMode::Mux },
    }).unwrap();
    assert_eq!(project.name, "Interview cut");
    assert_eq!(project.render_settings.width, 3840);
    assert_eq!(project.render_settings.captions, CaptionRenderMode::Mux);
}

#[test]
fn update_project_settings_rejects_blank_name_without_partial_mutation() {
    let mut project = fixtures::sample_project();
    let before = project.clone();
    assert!(apply_project_action(&mut project, ProjectAction::UpdateProjectSettings {
        name: "  ".into(), render_settings: before.render_settings.clone(),
    }).is_err());
    assert_eq!(project, before);
}
```

```tsx
it("submits one project settings action", () => {
  const onApply = vi.fn();
  render(<ProjectSettings project={project} projectDir="/Projects/interview" onApply={onApply} />);
  fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Interview cut" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply project settings" }));
  expect(onApply).toHaveBeenCalledWith(expect.objectContaining({ type: "updateProjectSettings", name: "Interview cut" }));
});
```

- [ ] **Step 2: Run action and component tests to confirm red state**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project::action::tests::update_project_settings -- --nocapture`

Expected: missing action variant.

Run: `rtk pnpm exec vitest run src/components/settings/project-settings.test.tsx src/lib/project.test.ts`

Expected: missing component/action adapter.

- [ ] **Step 3: Implement Project Settings and one atomic action**

Validate name, even dimensions, supported FPS, finite LUFS, and caption mode before mutation. Persist through the split-project mutation coordinator, refresh active project in `App`, and show scoped Storage plus compact Agent Guidance. Healthy skills collapse to one row; damaged skills expose existing exact repair preview.

```ts
export type UpdateProjectSettingsAction = {
  type: "updateProjectSettings";
  name: string;
  renderSettings: VideoProject["renderSettings"];
};
```

- [ ] **Step 4: Verify persistence, rollback, and UI behavior**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project::action::tests::update_project_settings -- --nocapture`

Expected: valid in-memory updates commit atomically and invalid updates leave the project unchanged.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project::split::tests::update_project_settings -- --nocapture`

Expected: valid update persists all split sidecars; invalid update leaves all files unchanged.

Run: `rtk pnpm exec vitest run src/components/settings/project-settings.test.tsx src/lib/project.test.ts`

Expected: format/output, storage, guidance, validation, and focus tests pass.

- [ ] **Step 5: Commit Task 9**

```bash
rtk git add src/components/settings/project-settings.tsx src/components/settings/project-settings.test.tsx src-tauri/src/project/action.rs src-tauri/src/project/split.rs src-tauri/src/main.rs src/lib/project.ts src/lib/project.test.ts src/App.tsx
rtk git commit -m "feat(settings): add project settings"
```

### Task 10: Contextual Missing-Configuration Notices

**Files:**
- Create: `src/components/settings/configuration-notice.tsx`
- Create: `src/components/settings/configuration-notice.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: Task 4 `AppSettingsTarget`, readiness for active model/provider/backend.
- Produces: `ConfigurationNotice` and `onOpenSettings(target, originElement)` from editor surfaces.

- [ ] **Step 1: Write failing notice and editor-gate tests**

```tsx
it("names the missing provider key and opens its exact settings row", () => {
  const onConfigure = vi.fn();
  render(<ConfigurationNotice message="OpenAI image generation needs an OpenAI provider key." target={{ category: "integrations", provider: "openai" }} onConfigure={onConfigure} />);
  const button = screen.getByRole("button", { name: "Configure OpenAI in Settings" });
  fireEvent.click(button);
  expect(onConfigure).toHaveBeenCalledWith({ category: "integrations", provider: "openai" }, button);
});

it("replaces a dead generation control with a configured path", () => {
  renderEditor({ selectedModel: "openai:gpt-image-2", openAiConfigured: false });
  expect(screen.getByText("OpenAI image generation needs an OpenAI provider key.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Configure OpenAI in Settings" })).toBeEnabled();
});
```

- [ ] **Step 2: Run focused notice/editor tests and confirm red state**

Run: `rtk pnpm exec vitest run src/components/settings/configuration-notice.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx`

Expected: missing notice component and existing generic setup errors fail.

- [ ] **Step 3: Implement typed notices for real blockers**

```tsx
export function ConfigurationNotice({ message, target, label, onConfigure }: Props) {
  return (
    <div role="status" className="flex items-start justify-between gap-3 border-l-2 border-amber-500 pl-3 text-xs">
      <span>{message}</span>
      <Button type="button" variant="outline" size="sm" onClick={(event) => onConfigure(target, event.currentTarget)}>
        {label}
      </Button>
    </div>
  );
}
```

Cover missing transcription model, no enabled generation models, missing selected-provider Keychain key, and selected-but-unconfigured Temporal. Closing Settings re-fetches preferences/provider/model readiness and restores focus.

- [ ] **Step 4: Verify all blocker/deep-link paths**

Run: `rtk pnpm exec vitest run src/components/settings/configuration-notice.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx src/App.test.tsx`

Expected: exact copy, targets, re-evaluation, and focus return pass; no disabled control lacks an adjacent explanation in covered workflows.

- [ ] **Step 5: Commit Task 10**

```bash
rtk git add src/components/settings/configuration-notice.tsx src/components/settings/configuration-notice.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx src/App.tsx src/App.test.tsx
rtk git commit -m "feat(settings): link blocked actions to configuration"
```

### Task 11: Native Menu, Focus, and Window Chrome Cleanup

**Files:**
- Modify: `src-tauri/src/native_menu.rs`
- Modify: `src/lib/native-menu.ts`
- Modify: `src/lib/native-menu.test.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/components/workspace/project-home.tsx`
- Modify: `src/components/workspace/project-home.test.tsx`

**Interfaces:**
- Consumes: App/Project/System surface openers from Tasks 4, 8, and 9.
- Produces: native `openProjectSettings` and `openSystemHealth` commands with correct enabled state.

- [ ] **Step 1: Write failing native-command and chrome tests**

```rust
#[test]
fn menu_has_scoped_settings_without_dead_update_or_save_items() {
    let ids = menu_item_ids_for_test(NativeMenuView::Editor);
    assert!(ids.contains("file.projectSettings"));
    assert!(ids.contains("help.systemHealth"));
    assert!(!ids.contains("app.checkUpdates"));
    assert!(!ids.contains("file.save"));
    assert!(!ids.contains("file.saveAs"));
}
```

```tsx
it("does not render simulated window controls", () => {
  render(<ProjectHome {...props} />);
  expect(screen.queryByLabelText("Window controls")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run focused native/frontend tests and confirm red state**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml native_menu::tests -- --nocapture`

Expected: old update/save commands remain and scoped commands are absent.

Run: `rtk pnpm exec vitest run src/lib/native-menu.test.ts src/App.test.tsx src/components/workspace/project-home.test.tsx`

Expected: old command union and fake traffic lights fail.

- [ ] **Step 3: Implement scoped commands and remove dead chrome**

Add Project Settings under File only when an editor project is active, System Health under Help, and contextual Advanced/Project Guidance links. Remove update and permanently disabled save items after auditing every canonical mutation path for immediate persistence. Remove the Project Home Circle traffic-light group.

- [ ] **Step 4: Verify menu state, command routing, and focus**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml native_menu::tests -- --nocapture`

Expected: native menu structure and enablement tests pass.

Run: `rtk pnpm exec vitest run src/lib/native-menu.test.ts src/App.test.tsx src/components/workspace/project-home.test.tsx`

Expected: commands route to exact surfaces, closing restores focus, and only native traffic lights remain visually.

- [ ] **Step 5: Commit Task 11**

```bash
rtk git add src-tauri/src/native_menu.rs src/lib/native-menu.ts src/lib/native-menu.test.ts src/App.tsx src/App.test.tsx src/components/workspace/project-home.tsx src/components/workspace/project-home.test.tsx
rtk git commit -m "fix(app): align native settings commands"
```

### Task 12: Build Warning and Settings Test-Debt Cleanup

**Files:**
- Modify: `vite.config.ts`
- Modify: `src/vite-build-policy.test.ts`
- Modify: `src/settings-visual-qa-scenarios.test.ts`
- Modify: `src/settings-native-e2e-script.test.ts`
- Modify: `src/lib/settings-visual-qa-fixtures.ts`
- Modify: `src/lib/settings-visual-qa-fixtures.test.ts`
- Modify: `src/lib/settings-acceptance-runner.ts`
- Modify: `src/lib/settings-acceptance-runner.test.ts`
- Delete: obsolete Settings page tests only after their behavior is covered by the new scoped tests.

**Interfaces:**
- Consumes: completed frontend surface structure.
- Produces: warning-free production chunk graph and current deterministic settings fixtures.

- [ ] **Step 1: Rewrite the chunk-policy test red-first**

```ts
it("does not manually split mutually dependent first-party editor modules", () => {
  const config = readFileSync("vite.config.ts", "utf8");
  expect(config).not.toContain('return "app-timeline"');
  expect(config).not.toContain('return "app-editor"');
  expect(config).toContain('return "vendor-react"');
  expect(config).toContain('return "vendor-tauri"');
});
```

- [ ] **Step 2: Run policy/build and capture the current warning**

Run: `rtk pnpm exec vitest run src/vite-build-policy.test.ts`

Expected: policy test fails while first-party manual chunks remain.

Run: `rtk pnpm build`

Expected before fix: build succeeds but prints `Circular chunk: app-timeline -> app-editor -> app-timeline`.

- [ ] **Step 3: Keep vendor splitting and let Rollup own first-party boundaries**

```ts
function productionManualChunks(moduleId: string): string | undefined {
  const id = moduleId.replace(/\\/g, "/");
  if (!id.includes("/node_modules/")) return undefined;
  if (id.includes("/react/") || id.includes("/react-dom/") || id.includes("/scheduler/")) return "vendor-react";
  if (id.includes("/lucide-react/")) return "vendor-icons";
  if (id.includes("/@tauri-apps/")) return "vendor-tauri";
  return "vendor";
}
```

Update visual fixtures and acceptance scenarios to the six App Settings categories plus Project Settings/System Health. Remove assertions for old sidebar badges, environment fallback, global project rows, and dead updater controls.

- [ ] **Step 4: Verify warning-free build and complete frontend suite**

Run: `rtk pnpm exec vitest run src/vite-build-policy.test.ts src/settings-visual-qa-scenarios.test.ts src/settings-native-e2e-script.test.ts`

Expected: policies and updated Settings scenarios pass.

Run: `rtk pnpm lint`

Expected: strict TypeScript passes.

Run: `rtk pnpm test`

Expected: full frontend suite passes with no new React act warnings.

Run: `rtk pnpm build`

Expected: production build passes without a circular chunk warning.

- [ ] **Step 5: Commit Task 12**

```bash
rtk git add vite.config.ts src/vite-build-policy.test.ts src/settings-visual-qa-scenarios.test.ts src/settings-native-e2e-script.test.ts src/lib/settings-visual-qa-fixtures.ts src/lib/settings-visual-qa-fixtures.test.ts src/lib/settings-acceptance-runner.ts src/lib/settings-acceptance-runner.test.ts
rtk git commit -m "fix(build): remove circular editor chunks"
```

### Task 13: Native Acceptance, Packaging, and Final Review

**Files:**
- Modify: `src/settings-native-e2e-script.test.ts`
- Modify: `src/lib/settings-acceptance-runner.ts`
- Modify: `src/lib/settings-acceptance-runner.test.ts`
- Modify: `src/settings-visual-qa-scenarios.test.ts`
- Create/update: `output/settings-architecture-rewrite/` native evidence (ignored artifacts)
- Modify: `.superpowers/sdd/progress.md` with concise completed task checkpoints during execution.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: fresh packaged-app evidence, full verification results, and no unresolved review findings.

- [ ] **Step 1: Add failing acceptance assertions for the final product contract**

```ts
expect(report.appSettings.categories).toEqual([
  "general", "projects", "aiModels", "integrations", "storage", "advanced",
]);
expect(report.appSettings.sidebarHealthBadges).toBe(0);
expect(report.providers.environmentCredentialControls).toBe(0);
expect(report.models.selectionControl).toBe("multiselect-combobox");
expect(report.blockedActions.every((action) => action.configureTargetResolved)).toBe(true);
expect(report.systemHealth.optionalProviderCountedAsRequired).toBe(false);
```

- [ ] **Step 2: Run acceptance tests and confirm missing final evidence**

Run: `rtk pnpm exec vitest run src/lib/settings-acceptance-runner.test.ts src/settings-native-e2e-script.test.ts src/settings-visual-qa-scenarios.test.ts`

Expected: new final-contract assertions fail until the runner/scenarios cover every surface.

- [ ] **Step 3: Complete deterministic and native acceptance coverage**

Build the fresh debug bundle, launch `/Users/olhapi/Documents/video-creater/src-tauri/target/debug/bundle/macos/Video Creater.app`, and drive the native app. Capture at the same viewport:

1. every App Settings category with no project;
2. model combobox open and filtered;
3. configured and unconfigured Keychain integrations;
4. missing-provider editor notice and exact deep link;
5. real Project Settings;
6. healthy and locally failed System Health;
7. focus return after Settings closes;
8. Project Home with only native traffic lights.

Record paths and results in the acceptance report. Do not use browser screenshots as native evidence.

- [ ] **Step 4: Run proportional full verification**

Run: `rtk pnpm lint`

Expected: strict TypeScript passes.

Run: `rtk pnpm test`

Expected: full frontend suite passes.

Run: `rtk pnpm build`

Expected: production build passes without circular chunk warnings.

Run: `rtk pnpm rust:fmt`

Expected: Rust formatting check passes.

Run: `rtk pnpm rust:clippy`

Expected: Clippy passes with no new suppression.

Run: `rtk pnpm rust:test:native`

Expected: full native Rust lane passes, or any platform-only deferral is isolated, reproduced, and explicitly recorded without hiding focused failures.

Run: `rtk pnpm verify:gstreamer-runtime`

Expected: the freshly bundled required GStreamer/GES runtime and plugin policy pass.

- [ ] **Step 5: Review the entire change against the spec**

Inspect `89beab1e..HEAD` for:

- every design requirement mapped to an implemented test;
- no stale Settings category, badge, environment fallback, updater control, or fake traffic light;
- no disabled implemented action without an adjacent configuration path;
- no secret-shaped field crossing IPC;
- no unintended package-manager runtime dependency;
- no TypeScript relaxation or Clippy suppression;
- no unrelated work staged.

Fix every finding red-first and rerun the affected focused and full gates.

- [ ] **Step 6: Commit final acceptance evidence and progress**

```bash
rtk git add src/settings-native-e2e-script.test.ts src/lib/settings-acceptance-runner.ts src/lib/settings-acceptance-runner.test.ts src/settings-visual-qa-scenarios.test.ts .superpowers/sdd/progress.md
rtk git commit -m "test(settings): verify native architecture rewrite"
```

## Completion Definition

The work is complete only when all thirteen tasks are committed on `main`, the fresh native Tauri app has been walked through and captured, required runtime packaging passes, all actionable review findings are fixed, and `.superpowers/sdd/progress.md` contains a concise final checkpoint rather than historical execution logs.
